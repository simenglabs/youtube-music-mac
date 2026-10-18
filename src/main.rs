use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use serde_json::json;
use tao::dpi::{LogicalSize, PhysicalPosition};
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tao::platform::macos::{ActivationPolicy, EventLoopExtMacOS, WindowBuilderExtMacOS};
use tao::window::{Window, WindowBuilder};
use tray_icon::{MouseButton, MouseButtonState, Rect, TrayIconBuilder, TrayIconEvent};
use window_vibrancy::{NSVisualEffectMaterial, apply_vibrancy};
use wry::WebViewBuilder;

const YT_MUSIC_URL: &str = "https://music.youtube.com";
// ponytail: hardcoded to Vivaldi; Chrome/Brave/Edge share the same AppleScript
// dictionary, so swapping the bundle id is enough. Safari needs `do JavaScript`.
const BROWSER_ID: &str = "com.vivaldi.vivaldi";
const POPOVER_HTML: &str = include_str!("popover.html");

// Scripts use a literal BROWSER_ID placeholder: `tell` needs a literal app id
// at compile time to resolve the browser's terminology (tabs, execute).

// Runs argv[1] as JavaScript in the first YT Music tab and returns its result.
// Guarded by `is running` so background polling never launches the browser.
const RUN_IN_TAB: &str = r#"
on run argv
  if application id "BROWSER_ID" is not running then return "NO_TAB"
  tell application id "BROWSER_ID"
    repeat with w in windows
      repeat with t in tabs of w
        if URL of t starts with "https://music.youtube.com" then
          return execute t javascript (item 1 of argv)
        end if
      end repeat
    end repeat
  end tell
  return "NO_TAB"
end run
"#;

// Brings the existing YT Music tab to front, or reports that none exists.
const FOCUS_TAB: &str = r#"
on run argv
  if application id "BROWSER_ID" is not running then return "NO_TAB"
  tell application id "BROWSER_ID"
    repeat with w in windows
      set i to 0
      repeat with t in tabs of w
        set i to i + 1
        if URL of t starts with "https://music.youtube.com" then
          set active tab index of w to i
          set index of w to 1
          activate
          return "OK"
        end if
      end repeat
    end repeat
  end tell
  return "NO_TAB"
end run
"#;

// Runs inside the YT Music tab: `(TAB_JS)(cmd, arg)` performs cmd, and for
// "state" returns the player state as JSON. AppleScript JS runs in an isolated
// world, so only the DOM is reachable (no YT internals like movie_player).
// ponytail: YT Music DOM selectors, verified 2026-10; update when YouTube redesigns.
const TAB_JS: &str = r#"(cmd, arg) => {
  const $ = s => document.querySelector(s);
  const btn = c => $('ytmusic-miniplayer .ytmusicPlayerControls' + c + 'Button button');
  const video = $('video');
  const volume = $('ytmusic-miniplayer input[aria-label="Volume"]');
  // Each queue entry may hold a song and a video version; take the shown one.
  const items = [...document.querySelectorAll('#queue #contents > *, #queue #automix-contents > *')].map(e => {
    const xs = e.matches('ytmusic-player-queue-item') ? [e] : [...e.querySelectorAll('ytmusic-player-queue-item')];
    return xs.find(i => i.offsetParent) || xs[xs.length - 1];
  }).filter(Boolean);
  const cur = items.findIndex(i => (i.getAttribute('play-button-state') || 'default') !== 'default');
  const httpsOnly = src => (src && src.startsWith('https://')) ? src : '';
  // Playlists: library ones from the sidebar guide (rendered twice, so dedupe), plus
  // playlists/albums/mixes on Home shelves (skipping single songs and podcasts).
  // ponytail: Home shelves only exist while the tab shows Home; a cache keyed by
  // playlist id would keep them after navigating away.
  const lists = [], seen = new Set();
  const add = (section, title, sub, play, img) => {
    if (!play || seen.has(title + '|' + sub)) return;
    seen.add(title + '|' + sub);
    lists.push({ section, title, sub, play, img: httpsOnly(img) });
  };
  for (const e of document.querySelectorAll('ytmusic-guide-entry-renderer')) {
    add('Library', e.querySelector('.title')?.textContent?.trim() || '', e.querySelector('.subtitle')?.textContent?.trim() || '',
      e.querySelector('ytmusic-play-button-renderer'), '');
  }
  for (const shelf of document.querySelectorAll('ytmusic-carousel-shelf-renderer')) {
    const section = shelf.querySelector('.title')?.textContent?.trim() || '';
    for (const it of shelf.querySelectorAll('ytmusic-two-row-item-renderer')) {
      const href = it.querySelector('a')?.getAttribute('href') || '';
      const sub = it.querySelector('.subtitle')?.textContent?.replace(/\s+/g, ' ').trim() || '';
      if (!/list=|browse\//.test(href) || /^(Song|Video|Episode)\b/.test(sub)) continue;
      add(section, it.querySelector('.title')?.textContent?.trim() || '', sub, it.querySelector('ytmusic-play-button-renderer'), it.querySelector('img')?.src);
    }
  }
  switch (cmd) {
    case 'play': btn('PlayPause')?.click(); break;
    case 'next': btn('Next')?.click(); break;
    case 'prev': btn('Previous')?.click(); break;
    case 'shuffle': btn('Shuffle')?.click(); break;
    case 'repeat': btn('Repeat')?.click(); break;
    case 'seek': if (video) video.currentTime = arg; break;
    case 'volume':
      if (volume) {
        volume.value = arg;
        volume.dispatchEvent(new Event('input', { bubbles: true }));
        volume.dispatchEvent(new Event('change', { bubbles: true }));
      }
      break;
    case 'playAt': items[arg]?.querySelector('ytmusic-play-button-renderer')?.click(); break;
    case 'playlist': lists[arg]?.play.click(); break;
  }
  if (cmd !== 'state') return '';
  const m = navigator.mediaSession.metadata;
  const repeat = btn('Repeat')?.getAttribute('aria-label') || '';
  return JSON.stringify({
    title: m?.title || '',
    artist: m?.artist || '',
    album: m?.album || '',
    art: httpsOnly(m?.artwork?.[m.artwork.length - 1]?.src),
    t: video?.currentTime || 0,
    d: video?.duration || 0,
    paused: video ? video.paused : true,
    vol: volume ? +volume.value : Math.round((video?.volume ?? 1) * 100),
    shuffle: btn('Shuffle')?.getAttribute('aria-pressed') === 'true',
    repeat: /one/i.test(repeat) ? 'one' : /all/i.test(repeat) ? 'all' : 'off',
    queue: items.slice(cur + 1, cur + 31).map((i, k) => ({
      i: cur + 1 + k,
      title: i.querySelector('.song-title')?.textContent?.trim() || '',
      artist: i.querySelector('.byline')?.textContent?.trim() || '',
      img: httpsOnly(i.querySelector('img')?.src),
    })),
    playlists: lists.map(({ section, title, sub, img }, i) => ({ i, section, title, sub, img })),
  });
}"#;

const LOGIN_LABEL: &str = "com.menglabs.yt-music";

fn login_plist_path() -> PathBuf {
    let home = std::env::var_os("HOME").unwrap_or_default();
    PathBuf::from(home).join(format!("Library/LaunchAgents/{LOGIN_LABEL}.plist"))
}

fn login_plist(exe: &str) -> String {
    let exe = exe.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key><string>{LOGIN_LABEL}</string>
  <key>ProgramArguments</key><array><string>{exe}</string></array>
  <key>RunAtLoad</key><true/>
  <key>ProcessType</key><string>Interactive</string>
</dict>
</plist>
"#
    )
}

/// "Open at Login" = a LaunchAgent plist pointing at this executable; launchd
/// picks it up at next login. Writing it again repoints it after the app moves.
fn set_open_at_login(on: bool) -> std::io::Result<()> {
    let path = login_plist_path();
    if !on {
        return match std::fs::remove_file(&path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        };
    }
    let exe = std::env::current_exe()?;
    std::fs::create_dir_all(path.parent().unwrap())?;
    std::fs::write(&path, login_plist(&exe.to_string_lossy()))
}

const POPOVER_W: f64 = 380.0;
const POPOVER_H: f64 = 700.0;

enum TabResult {
    Ok(String),
    NoTab,
    Err(String),
}

fn osascript(script: &str, args: &[&str]) -> TabResult {
    let script = script.replace("BROWSER_ID", BROWSER_ID);
    let out = match Command::new("osascript").arg("-e").arg(script).args(args).output() {
        Ok(out) => out,
        Err(e) => return TabResult::Err(e.to_string()),
    };
    if !out.status.success() {
        return TabResult::Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s == "NO_TAB" { TabResult::NoTab } else { TabResult::Ok(s) }
}

fn tab_call(cmd: &str, arg: f64) -> TabResult {
    // cmd comes from parse_command's whitelist, so {:?} is a safe JS string literal.
    osascript(RUN_IN_TAB, &[&format!("({TAB_JS})({cmd:?}, {arg})")])
}

fn focus_or_open_tab() {
    if let TabResult::NoTab = osascript(FOCUS_TAB, &[]) {
        let _ = Command::new("open").args(["-b", BROWSER_ID, YT_MUSIC_URL]).status();
    }
}

/// Player state JSON for the popover, or `{"error": kind}` the UI turns into a setup guide.
fn state_json(result: TabResult) -> String {
    match result {
        TabResult::Ok(s) if s.starts_with('{') => s,
        TabResult::Ok(_) | TabResult::NoTab => json!({ "error": "no_tab" }).to_string(),
        TabResult::Err(e) if e.contains("turned off") => json!({ "error": "js_off" }).to_string(),
        TabResult::Err(e) if e.contains("-1743") || e.contains("Not authorized") => {
            json!({ "error": "automation" }).to_string()
        }
        TabResult::Err(e) => json!({ "error": "other", "message": e }).to_string(),
    }
}

/// "♫ Title — Artist" for the menubar, cut to stay compact; just "♫" when idle.
fn menubar_title(state: &str) -> String {
    let v: serde_json::Value = serde_json::from_str(state).unwrap_or_default();
    let title = v["title"].as_str().unwrap_or("");
    let artist = v["artist"].as_str().unwrap_or("");
    let song = match (title, artist) {
        ("", _) => return "♫".to_string(),
        (t, "") => t.to_string(),
        (t, a) => format!("{t} — {a}"),
    };
    let max = 32;
    if song.chars().count() <= max {
        format!("♫ {song}")
    } else {
        format!("♫ {}…", song.chars().take(max - 1).collect::<String>().trim_end())
    }
}

/// Parses popover IPC like "seek:42.5" into a whitelisted tab command.
fn parse_command(msg: &str) -> Option<(&'static str, f64)> {
    let (name, arg) = msg.split_once(':').unwrap_or((msg, "0"));
    let arg: f64 = arg.parse().ok().filter(|a: &f64| a.is_finite())?;
    let name = ["play", "next", "prev", "shuffle", "repeat", "seek", "volume", "playAt", "playlist"]
        .into_iter()
        .find(|c| *c == name)?;
    Some((name, arg))
}

/// Centers the popover under the menubar icon, kept inside the screen.
fn show_below(window: &Window, icon: Rect) {
    let size = window.outer_size();
    let mut x = icon.position.x + icon.size.width as f64 / 2.0 - size.width as f64 / 2.0;
    let y = icon.position.y + icon.size.height as f64 + 4.0;
    if let Some(m) = window.monitor_from_point(icon.position.x, icon.position.y) {
        let left = m.position().x as f64 + 8.0;
        let right = m.position().x as f64 + m.size().width as f64 - size.width as f64 - 8.0;
        x = x.min(right).max(left);
    }
    window.set_outer_position(PhysicalPosition::new(x, y));
    window.set_visible(true);
    window.set_focus();
}

enum UserEvent {
    Tray(TrayIconEvent),
    State(String),
    Ipc(String),
}

fn main() {
    // Keep an existing login item pointed at wherever this copy now lives.
    if login_plist_path().exists() {
        let _ = set_open_at_login(true);
    }

    let mut event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    // Hide Dock icon; live in menubar only.
    event_loop.set_activation_policy(ActivationPolicy::Accessory);

    let window = WindowBuilder::new()
        .with_title("YouTube Music")
        .with_inner_size(LogicalSize::new(POPOVER_W, POPOVER_H))
        .with_decorations(false)
        .with_transparent(true)
        .with_resizable(false)
        .with_always_on_top(true)
        .with_visible_on_all_workspaces(true)
        .with_has_shadow(true)
        .with_visible(false)
        .build(&event_loop)
        .unwrap();

    let ipc_proxy = event_loop.create_proxy();
    let webview = WebViewBuilder::new()
        .with_html(POPOVER_HTML)
        .with_transparent(true)
        .with_ipc_handler(move |req| {
            let _ = ipc_proxy.send_event(UserEvent::Ipc(req.into_body()));
        })
        .build(&window)
        .unwrap();
    let _ = apply_vibrancy(&window, NSVisualEffectMaterial::Popover, None, Some(16.0));

    let proxy = event_loop.create_proxy();
    TrayIconEvent::set_event_handler(Some(move |e| {
        let _ = proxy.send_event(UserEvent::Tray(e));
    }));

    // Poll tab state every 1s while the popover is open, every 3s otherwise (for
    // the menubar title); a wake message forces an immediate refresh.
    let visible = Arc::new(AtomicBool::new(false));
    let (wake, wake_rx) = mpsc::channel::<()>();
    {
        let visible = visible.clone();
        let proxy = event_loop.create_proxy();
        thread::spawn(move || {
            loop {
                let _ = proxy.send_event(UserEvent::State(state_json(tab_call("state", 0.0))));
                let wait = if visible.load(Ordering::Relaxed) { 1 } else { 3 };
                let _ = wake_rx.recv_timeout(Duration::from_secs(wait));
            }
        });
    }

    // ponytail: text title instead of icon image, add PNG icon when branding matters.
    // Tray must be created after event loop starts on macOS, so build it lazily.
    let mut tray = None;
    let mut tray_title = String::new();
    // Clicking the icon while open first blurs the popover; don't reopen it right away.
    let mut hidden_at = Instant::now();
    // YT_MUSIC_OPEN=1 opens the popover on launch and keeps it pinned (for screenshots / quick checks).
    let pinned = std::env::var_os("YT_MUSIC_OPEN").is_some();
    let mut open_on_launch = pinned;

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        let tray = tray.get_or_insert_with(|| {
            TrayIconBuilder::new()
                .with_title("♫")
                .with_tooltip("YouTube Music")
                .build()
                .unwrap()
        });
        // Status item sits at the screen origin until macOS places it in the
        // menubar, so wait (re-checking every 100ms) for a real top-of-screen rect.
        if open_on_launch {
            match tray.rect().filter(|r| r.position.x > 0.0 && r.position.y < 100.0) {
                Some(rect) => {
                    open_on_launch = false;
                    show_below(&window, rect);
                    visible.store(true, Ordering::Relaxed);
                    let _ = wake.send(());
                }
                None => {
                    *control_flow = ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(100))
                }
            }
        }

        let mut hide = |window: &Window| {
            window.set_visible(false);
            visible.store(false, Ordering::Relaxed);
            hidden_at = Instant::now();
        };

        match event {
            Event::UserEvent(UserEvent::Tray(TrayIconEvent::Click {
                rect,
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            })) => {
                if window.is_visible() {
                    hide(&window);
                } else if hidden_at.elapsed() > Duration::from_millis(300) {
                    show_below(&window, rect);
                    visible.store(true, Ordering::Relaxed);
                    let _ = wake.send(());
                    let on = login_plist_path().exists();
                    let _ = webview.evaluate_script(&format!("window.setLogin({on})"));
                }
            }
            Event::WindowEvent {
                event: WindowEvent::Focused(false),
                ..
            } if !pinned => hide(&window),
            Event::UserEvent(UserEvent::State(state)) => {
                let title = menubar_title(&state);
                if title != tray_title {
                    tray.set_title(Some(&title));
                    tray_title = title;
                }
                let literal = serde_json::to_string(&state).unwrap();
                let _ = webview.evaluate_script(&format!("window.update({literal})"));
            }
            Event::UserEvent(UserEvent::Ipc(msg)) => match msg.as_str() {
                "hide" => hide(&window),
                "quit" => *control_flow = ControlFlow::Exit,
                "open" => {
                    hide(&window);
                    thread::spawn(focus_or_open_tab);
                }
                "login:1" | "login:0" => {
                    if let Err(e) = set_open_at_login(msg == "login:1") {
                        eprintln!("open at login: {e}");
                    }
                    let on = login_plist_path().exists();
                    let _ = webview.evaluate_script(&format!("window.setLogin({on})"));
                }
                "automation" => {
                    let _ = Command::new("open")
                        .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Automation")
                        .spawn();
                }
                _ => {
                    if let Some((cmd, arg)) = parse_command(&msg) {
                        let wake = wake.clone();
                        thread::spawn(move || {
                            tab_call(cmd, arg);
                            // Give YT Music a moment to update its buttons before re-reading.
                            thread::sleep(Duration::from_millis(250));
                            let _ = wake.send(());
                        });
                    }
                }
            },
            _ => {}
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_command_whitelists() {
        assert_eq!(parse_command("play"), Some(("play", 0.0)));
        assert_eq!(parse_command("seek:42.5"), Some(("seek", 42.5)));
        assert_eq!(parse_command("playAt:3"), Some(("playAt", 3.0)));
        assert_eq!(parse_command("playlist:1"), Some(("playlist", 1.0)));
        assert_eq!(parse_command("eval:1"), None);
        assert_eq!(parse_command("seek:NaN"), None);
        assert_eq!(parse_command("seek:1);alert(1"), None);
    }

    #[test]
    fn menubar_title_formats_and_truncates() {
        assert_eq!(menubar_title(r#"{"error":"no_tab"}"#), "♫");
        assert_eq!(menubar_title(r#"{"title":"","artist":"X"}"#), "♫");
        assert_eq!(menubar_title(r#"{"title":"Ropang","artist":"Bagus Faisal"}"#), "♫ Ropang — Bagus Faisal");
        assert_eq!(menubar_title(r#"{"title":"瞬き","artist":""}"#), "♫ 瞬き");
        let long = menubar_title(r#"{"title":"A very long song title that goes on","artist":"Someone"}"#);
        assert!(long.ends_with('…') && long.chars().count() <= 34, "{long}");
    }

    #[test]
    fn login_plist_escapes_path() {
        let p = login_plist("/Applications/A & B.app/Contents/MacOS/yt-music");
        assert!(p.contains("<string>/Applications/A &amp; B.app/Contents/MacOS/yt-music</string>"));
        assert!(p.contains(LOGIN_LABEL));
    }

    #[test]
    fn state_json_maps_errors() {
        assert_eq!(state_json(TabResult::Ok("{\"t\":1}".into())), "{\"t\":1}");
        assert!(state_json(TabResult::NoTab).contains("no_tab"));
        assert!(state_json(TabResult::Ok("missing value".into())).contains("no_tab"));
        assert!(state_json(TabResult::Err("… is turned off …".into())).contains("js_off"));
        assert!(state_json(TabResult::Err("Not authorized (-1743)".into())).contains("automation"));
    }
}
