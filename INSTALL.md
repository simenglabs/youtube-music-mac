# Panduan Install YT Music Bar

YT Music Bar adalah remote YouTube Music di menubar macOS. Musik tetap diputar di tab
YouTube Music di **Vivaldi**, jadi login akun Vivaldi kamu langsung dipakai.

Butuh: macOS 11 atau lebih baru, Vivaldi.

## 1. Pasang dari DMG

1. Buka file `YT-Music-Bar-<versi>.dmg`.
2. Seret **YT Music Bar** ke folder **Applications**.
3. Eject DMG.

## 2. Buka pertama kali

App ini belum ditandatangani Apple, jadi macOS menahannya saat pertama dibuka.

1. Buka **YT Music Bar** dari Applications. macOS akan bilang app tidak bisa dibuka. Klik **Done**.
2. Buka **System Settings › Privacy & Security**.
3. Scroll ke bawah, di bagian *Security* klik **Open Anyway** di samping "YT Music Bar".
4. Konfirmasi **Open Anyway** lalu masukkan password Mac.

Cara cepat lewat Terminal (sama hasilnya):

```sh
xattr -dr com.apple.quarantine "/Applications/YT Music Bar.app"
```

Setelah terbuka, ikon **♫** muncul di menubar (tidak ada ikon di Dock).

## 3. Izinkan Vivaldi dikontrol

Sekali saja:

1. Di Vivaldi buka **Settings** (`⌘ ,`) › **Privacy**.
2. Di bagian **Apple Events**, aktifkan **Allow JavaScript from Apple Events**.
3. Buka [music.youtube.com](https://music.youtube.com) di Vivaldi dan putar lagu.
4. Klik **♫** di menubar. macOS akan bertanya *"YT Music Bar wants to control Vivaldi"*: klik **Allow**.

Kalau terlanjur klik *Don't Allow*: **System Settings › Privacy & Security › Automation ›
YT Music Bar**, aktifkan **Vivaldi**. Popover juga menampilkan panduan ini bila izin belum ada.

## 4. Buka otomatis saat Mac menyala

Klik **♫** › tombol **⚙** › **Open at Login** (muncul tanda ✓).

## Update

Pasang DMG versi baru dengan cara yang sama (timpa app lama). Karena app belum
ditandatangani Apple, macOS mungkin menanyakan izin Vivaldi (langkah 3.4) lagi.

## Uninstall

1. **♫** › **⚙** › matikan **Open at Login**.
2. **⚙** › **Quit**.
3. Hapus **YT Music Bar** dari Applications.

## Masalah umum

| Gejala | Solusi |
| --- | --- |
| Popover: "Izinkan JavaScript dari Apple Events" | Langkah 3.1–3.2 |
| Popover: "Izinkan kontrol Vivaldi" | Aktifkan di System Settings › Privacy & Security › Automation |
| Popover: "Belum ada tab YouTube Music" | Buka music.youtube.com di Vivaldi (tombol di popover juga bisa) |
| Ikon ♫ tidak terlihat | Menubar penuh / tertutup notch. Kurangi ikon lain atau pakai Bartender/Ice |
| Tombol tidak bereaksi | YouTube mungkin mengubah tampilannya; buka issue di GitHub |
