# YT Music Bar

A YouTube Music remote for the macOS menubar, written in Rust. Click **♫** in the menubar
to see what's playing and control playback, volume, the queue, and playlists — without
switching to a browser window.

Music keeps playing in your [music.youtube.com](https://music.youtube.com) tab in
**Vivaldi**, so your account, playlists, and YouTube Premium work as-is — no extra sign-in.

<p align="center">
  <img src="docs/screenshots/player-light.png" width="280" alt="Player popover, light theme">
  <img src="docs/screenshots/player-dark.png" width="280" alt="Player popover, dark theme">
  <img src="docs/screenshots/playlists-light.png" width="280" alt="Playlists tab and settings menu">
</p>

> Screenshots use sample data (made-up titles, gradient covers).

## Features

- Current song in the menubar: `♫ Title — Artist`
- Cover art, title, artist, album, and a draggable progress bar (seek)
- Play/pause, previous, next, shuffle, repeat (off → all → one)
- Volume slider
- **Up Next**: upcoming songs — click one to play it right away
- **Playlists**: your Library playlists plus playlists, mixes, and albums from Home
- Follows the macOS light/dark theme, with a frosted-glass (vibrancy) popover
- **Open at Login** from the ⚙ menu
- Built-in setup guide in the popover when a permission is missing

## Download

**[⬇ YT-Music-Bar-0.1.0.dmg](https://github.com/simenglabs/youtube-music-mac/raw/main/releases/YT-Music-Bar-0.1.0.dmg)**
(2 MB, universal: Apple Silicon + Intel, macOS 11+)

SHA-256: `e6ae15a0f65e2e086a302737036c9cf7b69c05d636ef3966a0547f35b299ed60`

Requires [Vivaldi](https://vivaldi.com).

## Installation

<p align="center"><img src="docs/screenshots/dmg.png" width="420" alt="DMG contents"></p>

### 1. Install the app

1. Open `YT-Music-Bar-0.1.0.dmg`.
2. Drag **YT Music Bar** into **Applications**.
3. Eject the DMG.

### 2. Open it the first time

The app isn't signed by Apple yet, so macOS blocks the first launch.

1. Open **YT Music Bar** from Applications. When macOS says it can't be opened, click **Done**.
2. Go to **System Settings › Privacy & Security**.
3. Scroll down to *Security* and click **Open Anyway** next to "YT Music Bar".
4. Confirm with **Open Anyway** and your Mac password.

Or do the same from Terminal:

```sh
xattr -dr com.apple.quarantine "/Applications/YT Music Bar.app"
```

Once it's running, **♫** appears in the menubar (there's no Dock icon).

### 3. Let it control Vivaldi

One-time setup:

1. In Vivaldi, open **Settings** (`⌘ ,`) › **Privacy**.
2. Under **Apple Events**, enable **Allow JavaScript from Apple Events**.
3. Open [music.youtube.com](https://music.youtube.com) in Vivaldi and play something.
4. Click **♫** in the menubar. When macOS asks *"YT Music Bar wants to control Vivaldi"*,
   click **Allow**.

If you clicked *Don't Allow* by mistake: **System Settings › Privacy & Security ›
Automation › YT Music Bar** → turn on **Vivaldi**.

The popover shows these steps itself whenever a permission is missing:

<p align="center"><img src="docs/screenshots/setup-dark.png" width="280" alt="Setup guide inside the popover"></p>

### 4. Start at login

Click **♫** › **⚙** › **Open at Login** (a ✓ appears).

### Updating

Install the new DMG the same way, replacing the old app. Because the app is ad-hoc
signed, macOS may ask for the Vivaldi permission (step 3.4) again.

### Uninstalling

1. **♫** › **⚙** › turn off **Open at Login**.
2. **⚙** › **Quit**.
3. Delete **YT Music Bar** from Applications.

### Troubleshooting

| Symptom | Fix |
| --- | --- |
| Popover asks to allow JavaScript from Apple Events | Steps 3.1–3.2 |
| Popover asks to allow controlling Vivaldi | System Settings › Privacy & Security › Automation |
| Popover says there's no YouTube Music tab | Open music.youtube.com in Vivaldi (the popover has a button for it) |
| ♫ isn't visible | The menubar is full or hidden behind the notch; remove icons or use Bartender/Ice |
| Buttons stop responding | YouTube probably changed its page layout — please open an issue |

A guide in Indonesian is included in the DMG and at [INSTALL.md](INSTALL.md).

## How it works

```
menubar ♫ ──click──▶ popover (webview, src/popover.html)
                        │ IPC: "play", "seek:42", "playlist:3", …
                        ▼
                    Rust (src/main.rs) ──osascript──▶ Vivaldi
                                                      └─ execute javascript in the
                                                         music.youtube.com tab
```

- The app doesn't play audio or download streams itself. It presses buttons on, and reads
  the state of, the YouTube Music player in your Vivaldi tab via AppleScript.
- State is polled every second while the popover is open, and every 3 seconds while it's
  closed (for the menubar title).
- Commands from the popover are limited to a fixed whitelist plus a number. Text from
  YouTube is rendered as plain text, never as HTML.

## Building from source

Requires Rust (2024 edition) and the Xcode Command Line Tools.

```sh
cargo run --release              # run it
YT_MUSIC_OPEN=1 cargo run        # open the popover on launch and keep it pinned
cargo test                       # unit tests
./build-dmg.sh                   # universal app (arm64 + x86_64) → dist/*.dmg
```

`build-dmg.sh` needs both macOS targets:
`rustup target add aarch64-apple-darwin x86_64-apple-darwin`.

## Limitations

- **Vivaldi only.** Chrome, Brave, and Edge share the same AppleScript dictionary — change
  `BROWSER_ID` in `src/main.rs`. Safari needs a small change (`do JavaScript`).
- **No reordering or removing queue items.** YouTube Music's per-song menu can't be opened
  from outside the page.
- **Some Up Next thumbnails show ♪.** YouTube Music only loads queue thumbnails while the
  queue panel is visible. Open the full player page in the Vivaldi tab once to load them.
- **Recommended playlists** only show while the YouTube Music tab is on Home.
- Depends on YouTube Music's page structure. If YouTube changes its layout, the selectors
  in `TAB_JS` (`src/main.rs`) need updating.
- Ad-hoc signed, not notarized by Apple.

## Disclaimer

A personal project, not affiliated with Google or YouTube. YouTube Music is a trademark of
Google LLC.
