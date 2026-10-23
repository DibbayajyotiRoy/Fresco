<div align="center">

<img src="./data/icons/hicolor/256x256/apps/io.github.dibbayajyotiroy.Fresco.png" width="112" alt="Ikon aplikasi Fresco — aplikasi wallpaper hidup yang gratis dan open source untuk Linux" />

# Fresco — Aplikasi Wallpaper Bergerak dan Wallpaper Video Gratis untuk Linux

[English](README.md) | [বাংলা](README.bn.md) | [Deutsch](README.de.md) | [Español](README.es.md) | [Français](README.fr.md) | [हिन्दी](README.hi.md) | **Bahasa Indonesia** | [Português (Brasil)](README.pt-BR.md) | [Română](README.ro.md) | [Русский](README.ru.md) | [Türkçe](README.tr.md) | [Tiếng Việt](README.vi.md) | [简体中文](README.zh-CN.md)

**Fresco adalah aplikasi wallpaper hidup (live wallpaper) untuk Linux yang gratis dan open source. Dengan Fresco, video, GIF, atau gambar apa pun bisa menjadi wallpaper bergerak di desktop, baik di X11 maupun Wayland.** Fresco adalah alternatif Wallpaper Engine yang native untuk Linux, lengkap dengan aplikasi grafis, pemutaran berakselerasi GPU, dan widget desktop.

[![Rilis Fresco terbaru di GitHub](https://img.shields.io/github/v/release/DibbayajyotiRoy/fresco?style=flat-square&label=release)](https://github.com/DibbayajyotiRoy/fresco/releases/latest)
[![Lisensi GPL-3.0-or-later](https://img.shields.io/github/license/DibbayajyotiRoy/fresco?style=flat-square)](LICENSE)
[![Status workflow publikasi](https://img.shields.io/github/actions/workflow/status/DibbayajyotiRoy/fresco/publish.yml?style=flat-square&label=publish)](https://github.com/DibbayajyotiRoy/fresco/actions/workflows/publish.yml)
[![Bintang GitHub](https://img.shields.io/github/stars/DibbayajyotiRoy/fresco?style=flat-square)](https://github.com/DibbayajyotiRoy/fresco/stargazers)

[Situs web](https://fresco.dibbayajyoti.com) · [Instalasi](#instalasi) · [Lingkungan desktop yang didukung](#lingkungan-desktop-yang-didukung) · [Perbandingan](#perbandingan-fresco-dengan-wallpaper-engine-hidamari-komorebi-dan-mpvpaper) · [FAQ](#pertanyaan-yang-sering-diajukan-faq) · [Widget](#widget-desktop-lirik-jam-visualizer-dan-sampul-album) · [Changelog](CHANGELOG.md) · [Issues](https://github.com/DibbayajyotiRoy/fresco/issues)

<img src="./data/screenshots/gallery.png" alt="Pustaka wallpaper Fresco di desktop Linux bertema gelap: kisi wallpaper video, gambar, dan slideshow yang bisa dicari, dengan satu video bertanda Aktif dan label status yang menunjukkan dekode hardware NVDEC" width="800" />

</div>

## Apa itu Fresco?

Fresco mengubah video, GIF, gambar, slideshow, atau daftar putar menjadi wallpaper di belakang jendela-jendela Anda, lewat aplikasi GTK4 / libadwaita yang tidak memerlukan terminal. Pemutaran berjalan lewat mpv, sehingga dekode dikerjakan oleh GPU dan pemakaian CPU nyaris menganggur — biayanya terhadap daya bisa dilihat di bagian [Performa](#performa-dan-daya-tahan-baterai). Wallpaper Anda dipulihkan setiap kali login.

- **Yang bisa diputar:** video (mp4, webm, mkv, avi, mov), GIF animasi, gambar, slideshow gambar, dan daftar putar video.
- **Desktop:** sesi X11 (GNOME di Xorg, Cinnamon, Xfce, MATE, Deepin 25) dan Wayland (COSMIC, Hyprland, Sway, KDE Plasma 6, Cinnamon 6.8+). GNOME di Wayland hanya menampilkan satu bingkai diam — lihat [Lingkungan desktop yang didukung](#lingkungan-desktop-yang-didukung).
- **Dekode hardware:** VA-API (Intel, AMD) dan NVDEC (NVIDIA), lewat mpv.
- **Pustaka:** tautkan folder wallpaper yang sudah Anda punya alih-alih mengimpornya, dan jaga agar tetap sinkron saat berkas datang dan pergi. Pustaka hanya menggambar apa yang ada di layar, sehingga ribuan wallpaper tetap cepat dan ringan.
- **Widget:** lirik yang sinkron dengan musik, jam (termasuk jam dunia), visualizer audio, dan sampul album, semuanya digambar langsung di wallpaper. Semuanya nonaktif secara bawaan.
- **Kontrol:** aplikasinya sendiri, palet perintah Ctrl+K, serta `fresco next` / `fresco prev` / `fresco random` untuk pintasan keyboard.
- **Instalasi:** paket `.deb` untuk Ubuntu, Debian, Linux Mint, Pop!_OS, elementary OS, Kali, dan Deepin (skrip satu baris, unduh manual, atau Toko Aplikasi Deepin); di Fedora, Arch, dan distro lain, bangun dari kode sumber — lihat [Instalasi](#instalasi).
- **Lisensi:** gratis dan open source di bawah [GPL-3.0-or-later](LICENSE), tanpa iklan dan tanpa akun.
- **Pengguna:** 3.000+ orang di 128 negara. Versi terbaru: 1.1.50.

## Instalasi

Fresco dikemas sebagai `.deb` untuk distro keluarga Debian, dan dibangun dari kode sumber di tempat lain.

### Ubuntu, Debian, Linux Mint, Pop!_OS, elementary OS, dan Kali Linux

Instalasi satu baris — mengunduh `.deb` terbaru dari GitHub Releases lalu memasangnya dengan apt:

```bash
curl -fsSL https://github.com/DibbayajyotiRoy/fresco/releases/latest/download/install.sh | FRESCO_SOURCE=github bash
```

Atau unduh `.deb` dari [Releases](https://github.com/DibbayajyotiRoy/fresco/releases/latest) dan jalankan:

```bash
sudo apt install ./fresco_*.deb
```

### Deepin 25

Buka **Toko Aplikasi**, cari **Fresco**, lalu klik **Pasang**. Fresco tersedia di Toko Aplikasi Komunitas deepin, jadi tidak ada yang perlu diunduh manual dan pembaruan datang lewat toko tersebut. Instalasi satu baris di atas juga bisa dipakai di Deepin.

### Fedora, Arch Linux, dan distro lainnya

Belum ada paket siap pakai untuk distro-distro ini, jadi bangun dari kode sumber. Anda memerlukan Rust (lewat [rustup](https://rustup.rs)) beserta header pengembangan GTK4, libadwaita, dan libmpv:

```bash
git clone https://github.com/DibbayajyotiRoy/fresco.git && cd fresco
cargo build --release --all-features
```

Di compositor Wayland, Fresco juga memerlukan renderer `mpvpaper`, yang disertakan dalam `.deb` tetapi tidak dalam build dari kode sumber:

```bash
# Arch Linux, CachyOS, EndeavourOS (AUR)
yay -S mpvpaper

# Fedora dan distro lainnya: bangun terhadap libmpv sistem Anda
scripts/build-mpvpaper.sh
install -Dm755 target/release/mpvpaper ~/.local/bin/mpvpaper   # harus ada di PATH
```

Fresco tidak ada di AUR maupun Flathub — paket AUR bernama `fresco` adalah program fisika yang tidak berhubungan. Detail dan pemecahan masalah ada di [docs/INSTALL.md](docs/INSTALL.md).

Setelah terpasang, buka **Fresco** dari peluncur aplikasi Anda. Jika ada yang terlihat janggal, jalankan `fresco doctor`.

## Cara menjadikan video sebagai wallpaper di Linux

Pasang Fresco, klik **Tambah**, pilih video Anda, lalu klik **Jadikan wallpaper**. Video diputar di belakang jendela-jendela Anda dan kembali otomatis saat login.

1. Pasang Fresco (lihat di atas) dan buka dari peluncur aplikasi Anda.
2. Klik **Tambah** dan pilih video, GIF, atau gambar — atau tempel tautan lewat **Dari tautan**.
3. Jika perlu, pangkas atau putar di editor.
4. Klik **Jadikan wallpaper**, lalu tutup aplikasinya.

Wallpaper tetap diputar setelah jendela aplikasi ditutup.

## Fitur

- **Media apa pun** — video berulang (mp4/webm/mkv), GIF animasi, gambar statis, slideshow gambar, daftar putar beberapa video
- **Tambah dari tautan** — tempel pin Pinterest atau URL video/gambar langsung; Fresco mengunduhnya lalu membuka editor pangkas
- **Tautkan folder** — pakai folder wallpaper yang sudah Anda punya (gambar, GIF, dan video) tanpa mengimpornya; folder tetap sinkron saat Anda menambah dan menghapus berkas, subfolder bersifat opsional, dan favorit serta hasil suntingan Anda disimpan per berkas
- **Pustaka besar** — pustaka hanya menggambar wallpaper di dekat posisi gulir Anda, sehingga ribuan wallpaper terbuka dalam sekitar dua detik dan memakai beberapa ratus megabita (pada pustaka uji berisi 4.000 wallpaper: sekitar 2 dtk dan 325 MB, turun dari 9 sampai 13 dtk dan 1,6 GB)
- **Dekode hardware** — dekode video lewat GPU (VA-API / NVDEC) menjaga CPU tetap nyaris menganggur dengan memindahkan pekerjaan ke mesin video, tempat biayanya terukur lebih murah (bukan gratis — lihat [Performa](#performa-dan-daya-tahan-baterai))
- **Hemat daya** — penskalaan GPU yang lebih murah untuk laptop; terukur memangkas daya GPU hingga sekitar setengahnya (lihat [Performa](#performa-dan-daya-tahan-baterai))
- **Multi-monitor** — wallpaper berbeda untuk tiap layar, dengan pemutaran tersinkron untuk video yang sama di beberapa monitor
- **Jadwal siang dan malam** — ganti wallpaper berdasarkan timer, slot waktu bebas, atau matahari terbit/terbenam
- **Widget desktop** — lirik lagu yang sinkron, jam bertema, jam dunia, visualizer audio, dan cakram sampul album yang berputar, digambar langsung di wallpaper sehingga tidak ada yang menutupi jendela Anda (jam dunia punya mode opsional melayang di atas jendela); semuanya nonaktif secara bawaan (lihat [Widget desktop](#widget-desktop-lirik-jam-visualizer-dan-sampul-album))
- **Layar kunci** — tampilkan wallpaper dan widget (jam, ucapan salam, lagu yang sedang diputar, baterai, dan lainnya) di layar kunci sungguhan, menggantikan layar kunci bawaan — video hidup dan widget di COSMIC 1.9+ dan KDE Plasma 6, wallpaper beserta widget lewat `fresco lock` di Sway/Hyprland/niri dan X11, satu bingkai diam di tempat lain. Nonaktif secara bawaan, dan Fresco tidak pernah melihat kata sandi Anda — lihat [docs/LOCKSCREEN.md](docs/LOCKSCREEN.md)
- **Kontrol keyboard** — `fresco next`, `fresco prev`, dan `fresco random` untuk pintasan compositor (jendela Fresco yang terbuka mengikuti perubahan itu alih-alih mengembalikannya), serta palet perintah Ctrl+K di dalam aplikasi
- **Pengelolaan massal** — pilih beberapa wallpaper sekaligus dan hapus dalam satu langkah
- **Katalog bawaan** — jelajahi wallpaper pilihan yang berlisensi resmi langsung di dalam aplikasi
- **Jeda otomatis saat layar penuh** — per monitor, di X11, compositor wlroots, COSMIC, dan KDE Plasma (lewat plugin wallpapernya); opsional juga untuk jendela yang dimaksimalkan, ditambah jeda saat memakai baterai
- **Ekstensi tab baru browser** — tampilkan wallpaper Anda di setiap tab baru (Chrome/Brave/Edge/Firefox; muat sebagai ekstensi tanpa paket dari [`./extension`](extension))
- **Dukungan Deepin DDE** — di Deepin 25, Fresco menyesuaikan diri dengan desktop DDE secara otomatis, dan mengklik desktop memunculkan kembali ikon selama sepuluh detik kapan pun Anda memerlukannya (lihat [Pemecahan masalah](#ikon-desktop-saya-tersembunyi-saat-wallpaper-diputar-di-deepin))
- **Editor pangkas dan putar**, suara/volume per wallpaper, transisi slideshow, dan pustaka yang bisa dicari

## Lingkungan desktop yang didukung

| Lingkungan | Wallpaper hidup | Catatan |
|---|---|---|
| X11 (GNOME, Cinnamon, XFCE, …) | ✅ | Renderer tertanam. GNOME hanya jika distro masih menyertakan sesi Xorg (lihat catatan GNOME di bawah) |
| Kali Linux (Xfce, X11) | ✅ | Berjalan di sesi bawaan Kali, yaitu Xfce di X11, dengan renderer tertanam yang sama seperti desktop X11 lainnya |
| Xfce (X11) | ✅ | Ikon desktop tetap terlihat di atas wallpaper, dan Anda bisa mengeklik, memilih, serta menyeretnya |
| MATE (X11) | ✅ | Ikon desktop tetap terlihat dan bisa diklik di atas wallpaper |
| Deepin 25 (DDE, X11) | ✅ | Adaptasi DDE otomatis — diverifikasi komunitas di Deepin 25 Community build1 |
| COSMIC (Wayland) | ✅ | layer-shell |
| Hyprland | ✅ | layer-shell |
| Sway | ✅ | layer-shell |
| Cinnamon 6.8 ke atas (Wayland) | ✅ | Compositor Cinnamon mendapat layer-shell di versi 6.8. Cinnamon 6.6 ke bawah tidak punya layer-shell dan hanya menampilkan satu bingkai diam di Wayland |
| KDE Plasma 6 (X11 dan Wayland) | ✅ | Diatur lewat plugin wallpaper milik plasmashell sendiri, jadi ikon desktop tetap terlihat. Memerlukan `qml6-module-qtmultimedia` untuk video. Pemutaran memakai Qt, bukan mpv: tanpa suara, tanpa penyetelan hwdec, pangkas, atau transisi; daftar putar hanya memutar berkas pertamanya dan slideshow hanya menampilkan bingkai pertamanya. Opsi Metode pengisian (Penuhi, Sesuaikan, Regangkan) tetap berlaku |
| GNOME di Wayland | ⚠️ | Hanya satu bingkai diam — Mutter tidak menyediakan permukaan wallpaper hidup, jadi tidak ada video dan tidak ada widget. Video hidup memerlukan ekstensi GNOME untuk Fresco (direncanakan) |

Setiap lingkungan di atas diuji secara headless di CI pada setiap rilis.

**Catatan GNOME.** GNOME di Wayland adalah satu-satunya desktop arus utama tempat Fresco hanya bisa menampilkan satu bingkai diam. Jalan pintas lamanya, yaitu login ke sesi Xorg, mulai hilang: GNOME 49 menonaktifkan sesi X11-nya dan GNOME 50 menghapusnya, sehingga Ubuntu 25.10 dan yang lebih baru (termasuk 26.04 LTS) serta Fedora 43 dan yang lebih baru tidak lagi menyertakan sesi Xorg untuk dipilih. Ubuntu 22.04 dan 24.04 masih menyediakan "Ubuntu on Xorg". Di tempat yang tidak ada sesi Xorg, video hidup di desktop GNOME standar harus menunggu ekstensi GNOME untuk Fresco yang sudah direncanakan; Fresco menyampaikan hal ini di dalam aplikasi dan tidak berpura-pura videonya sedang diputar. KDE Plasma, COSMIC, Hyprland, Sway, dan semua desktop X11 tidak terpengaruh.

> "Mudah dipakai dengan antarmuka yang bersih — salah satu dari sedikit aplikasi wallpaper hidup yang benar-benar disesuaikan untuk Deepin 25, bisa dipasang lewat .deb dan berjalan mulus dengan pemutaran berakselerasi hardware."
>
> — 柒玖 (forum deepin) / 柒仈玖 (GitHub), diuji di Deepin 25 Community build1, sesi X11, Intel Alder Lake-N [Intel Graphics]

Deepin 25 menyertakan X11 sebagai sesi bawaannya, dan itulah sesi tempat Fresco diverifikasi di sana. Compositor Wayland milik Deepin sendiri, [Treeland](https://github.com/linuxdeepin/treeland), masih dalam pengembangan, jadi Fresco belum mengklaim apa pun tentang Deepin di Wayland.

## Perbandingan Fresco dengan Wallpaper Engine, Hidamari, Komorebi, dan mpvpaper

| Alat | Platform | Wayland | X11 | Antarmuka | Yang diputar | Lisensi |
|---|---|---|---|---|---|---|
| **Fresco** | Linux | ✅ compositor layer-shell dan KDE Plasma 6; satu bingkai diam di GNOME | ✅ | Aplikasi GTK4, plus CLI `fresco` | Video, GIF, gambar, slideshow, daftar putar | GPL-3.0-or-later |
| **Wallpaper Engine** | Windows (Steam); tidak ada versi Linux resmi | n/a | n/a | Aplikasi grafis | Wallpaper video, scene, dan web | Berbayar, sumber tertutup |
| **[linux-wallpaperengine](https://github.com/Almamu/linux-wallpaperengine)** | Linux; memerlukan aset dari salinan Wallpaper Engine di Steam | ✅ compositor dengan layer-shell | ⚠️ tidak berfungsi jika lingkungan desktop (GNOME, KDE) yang menggambar latar belakang | Command line; README-nya mencantumkan GUI komunitas yang terpisah | Wallpaper Wallpaper Engine | GPL-3.0 |
| **[Hidamari](https://github.com/jeffshee/hidamari)** | Linux, berfokus pada GNOME | ⚠️ GNOME Wayland | ✅ | Aplikasi grafis | Video | GPL-3.0 |
| **[Komorebi](https://github.com/cheesecakeufo/komorebi)** | Linux | — | — | Aplikasi grafis | Wallpaper gambar, video, dan halaman web | GPL-3.0 |
| **[mpvpaper](https://github.com/GhostNaN/mpvpaper)** | Linux, compositor berbasis wlroots | ✅ layer-shell | ❌ | Command line | Video, lewat mpv | GPL-3.0 |
| **xwinwrap** | Linux, hanya X11 | ❌ | ✅ | Command line | Apa pun yang Anda pasang (mpv, gifview) | — |
| **swww** (kini [awww](https://codeberg.org/LGFae/awww)) | Linux, compositor layer-shell; bukan GNOME | ✅ | ❌ | Daemon command line | GIF animasi dan gambar, bukan video | GPL-3.0 |

Fresco menyertakan `mpvpaper` sebagai renderer Wayland-nya, jadi Fresco dibangun di atas proyek itu, bukan bersaing dengannya. Tanda `—` berarti dokumentasi proyek tersebut tidak menyatakannya dengan jelas. Diperiksa terhadap repositori publik masing-masing proyek pada 2026-10-16: push terakhir ke repositori Komorebi terjadi pada Juli 2025, dan repositori GitHub swww sudah diarsipkan karena proyeknya pindah ke Codeberg dengan nama awww.

## Performa dan daya tahan baterai

柒玖 (forum deepin) / 柒仈玖 (GitHub) mengukur daya paket dengan `turbostat` pada Intel N150 (Deepin 25, VA-API, dua kali uji per tingkat) saat wallpaper video diputar:

| Video | Hemat daya | Daya GPU | Total daya paket |
|---|---|---|---|
| 1080p 60fps | Kualitas penuh | 1,37 W | 6,00 W |
| 1080p 60fps | **Rendah** (bawaan) | **0,63 W** (−54%) | **4,03 W** (−33%) |
| 4K 60fps | Kualitas penuh | 2,77 W | 7,94 W |
| 4K 60fps | **Rendah** (bawaan) | **1,60 W** (−42%) | **5,95 W** (−25%) |
| 4K 60fps | Minimum | 0,99 W (−65%) | 4,97 W (−37%) |

Hemat daya mengurangi biaya penskalaan GPU per bingkai. Tidak ada bingkai yang dibuang dan dekode hardware tidak disentuh, sehingga pemutaran tetap mulus — yang dikorbankan adalah ketajaman gambar, bukan gerakan. Rendah adalah pengaturan bawaan; Minimum layak dipilih untuk sumber 4K.

**Cara membaca angka-angka ini.** Dekode hardware tidak membuat wallpaper hidup menjadi gratis — pekerjaannya dipindahkan dari CPU ke mesin video, tempat biayanya lebih murah, bukan hilang. "Pemakaian CPU rendah" saja adalah klaim yang lemah, karena pembandingnya adalah dekode software yang jauh lebih buruk; ukuran yang jujur adalah konsumsi seluruh sistem, itulah sebabnya tabel melaporkan **total daya paket** dan bukan hanya daya GPU.

Dua catatan untuk angka di atas, supaya tidak dibaca lebih dari yang sebenarnya:

- Angka-angka itu adalah **total daya paket saat wallpaper diputar**, bukan biaya tambahan dari wallpaper itu sendiri. Tidak ada baseline saat idle yang dicatat di mesin yang sama, jadi selisih antara angka-angka ini dan desktop yang menganggur tidak dapat dipastikan di sini. Persentasenya membandingkan *tingkat* hemat daya satu sama lain, dan untuk itulah angka tersebut valid.
- Angka-angka itu berasal dari satu mesin (Intel N150, Alder Lake-N, VA-API, Deepin 25, dua kali uji per tingkat). GPU diskret, NVDEC, dan driver lain akan memberi hasil yang berbeda.

Wallpaper hidup selalu lebih boros daripada wallpaper statis. Hemat daya, jeda otomatis saat layar penuh, dan Jeda saat baterai ada untuk membatasi biaya itu, bukan untuk berpura-pura biayanya nol.

## Pertanyaan yang sering diajukan (FAQ)

### Apakah ada alternatif Wallpaper Engine untuk Linux?

Ada. Fresco adalah aplikasi Linux yang gratis dan open source yang memutar wallpaper video, GIF, dan gambar milik Anda sendiri lewat antarmuka grafis, dengan dekode hardware dan tanpa lapisan kompatibilitas. Fresco tidak membuka berkas wallpaper milik Wallpaper Engine; jika Anda memiliki Wallpaper Engine dan ingin memakainya di Linux, proyek terpisah [linux-wallpaperengine](https://github.com/Almamu/linux-wallpaperengine) menjalankannya dari command line dan memerlukan aset dari salinan Wallpaper Engine di Steam. Lihat [tabel perbandingan](#perbandingan-fresco-dengan-wallpaper-engine-hidamari-komorebi-dan-mpvpaper).

### Apakah Wallpaper Engine bisa dipakai di Linux?

Tidak secara native. Wallpaper Engine adalah aplikasi Windows tanpa versi Linux resmi. Fresco adalah alternatif native untuk Linux yang dipasang sebagai `.deb` dan tidak memerlukan lapisan kompatibilitas.

### Bagaimana cara menjadikan video sebagai wallpaper di Ubuntu?

Pasang Fresco, buka, klik **Tambah**, pilih video Anda, lalu klik **Jadikan wallpaper**. Video diputar sebagai latar belakang desktop dan dipulihkan saat login. Sesi bawaan Ubuntu, GNOME di Wayland, hanya bisa menampilkan satu bingkai diam. Di Ubuntu 22.04 dan 24.04 Anda bisa login ke sesi **Ubuntu on Xorg** untuk pemutaran hidup penuh; Ubuntu 25.10 dan yang lebih baru (termasuk 26.04 LTS) tidak menyertakan sesi Xorg, jadi di sana video hidup di desktop GNOME standar memerlukan ekstensi GNOME untuk Fresco yang masih direncanakan. Varian Ubuntu yang memakai KDE Plasma, Xfce, atau MATE tidak terpengaruh.

### Bagaimana cara memasang wallpaper hidup di Ubuntu, Linux Mint, Pop!_OS, Fedora, atau Arch Linux?

Pasang `.deb` Fresco di Ubuntu, Linux Mint, dan Pop!_OS, atau bangun dari kode sumber di Fedora dan Arch Linux. Perintah untuk masing-masing ada di [Instalasi](#instalasi).

- **Ubuntu:** `.deb`. Di desktop GNOME bawaan, pemutaran hidup memerlukan sesi Xorg (Ubuntu 22.04 dan 24.04); varian KDE Plasma, Xfce, dan MATE tidak terpengaruh — lihat jawaban tentang Ubuntu di atas.
- **Linux Mint:** `.deb`. Cinnamon di X11 didukung, dan Cinnamon di Wayland mulai versi 6.8.
- **Pop!_OS:** `.deb`. Desktop COSMIC (Wayland layer-shell) didukung.
- **Fedora:** bangun dari kode sumber. GNOME di Fedora 43 dan yang lebih baru hanya Wayland, jadi menampilkan satu bingkai diam; KDE Plasma dan desktop X11 didukung.
- **Arch Linux:** bangun dari kode sumber dan pasang `mpvpaper` dari AUR (`yay -S mpvpaper`).

Hanya jalur `.deb` yang dicakup oleh CI Fresco.

### Distro Linux apa saja yang didukung?

Fresco menyediakan paket `.deb` untuk distro berbasis Debian dan Ubuntu: Ubuntu, Pop!_OS, Linux Mint, Debian, elementary OS, Deepin 25, dan Kali Linux. Distro lain bisa membangun dari kode sumber — lihat [Instalasi](#instalasi) dan [docs/INSTALL.md](docs/INSTALL.md).

### Apakah wallpaper bergerak memakan banyak CPU, GPU, atau baterai?

CPU, tidak. Baterai, sedikit — wallpaper hidup tidak pernah gratis.

Fresco mendekode video di GPU (VA-API / NVDEC), sehingga pemakaian CPU nyaris menganggur. Itu memindahkan biayanya ke mesin video, bukan menghilangkannya. Diukur pada Intel N150 dengan tingkat Hemat daya bawaan, wallpaper 1080p memakai daya GPU 0,63 W dengan total daya paket 4,03 W saat diputar (lihat [Performa](#performa-dan-daya-tahan-baterai) untuk cara membacanya, termasuk apa yang *tidak* dikatakannya). Jeda otomatis saat layar penuh dan Jeda saat baterai ada untuk menjaga biaya itu tidak menguras baterai pada saat yang paling penting.

### Apakah Fresco berjalan di Wayland?

Ya, di compositor yang mengimplementasikan protokol layer-shell — COSMIC, Hyprland, dan Sway — serta di KDE Plasma 6 (X11 dan Wayland) lewat plugin wallpaper milik plasmashell sendiri. GNOME di Wayland adalah pengecualian: Mutter tidak menyediakan permukaan wallpaper, jadi Fresco menampilkan satu bingkai diam di sana dan menyampaikannya di dalam aplikasi. Sesi X11 didukung penuh.

### Apakah Fresco berjalan di GNOME?

Ya di X11, dengan wallpaper hidup penuh — video, widget, semuanya. Di **GNOME pada Wayland**, Mutter tidak menyediakan permukaan wallpaper yang bisa digambari aplikasi mana pun, jadi Fresco menampilkan satu bingkai diam di sana dan widget tidak tersedia. Jika distro Anda masih menyertakan sesi Xorg (Ubuntu 22.04 dan 24.04 menyertakannya), logout lalu pilih "GNOME on Xorg" atau "Ubuntu on Xorg" untuk pemutaran hidup penuh. GNOME 49 menonaktifkan sesi itu dan GNOME 50 menghapusnya, sehingga Ubuntu 25.10 dan yang lebih baru (termasuk 26.04 LTS) serta Fedora 43 dan yang lebih baru tidak memilikinya: di sana, video hidup di GNOME memerlukan ekstensi GNOME untuk Fresco yang masih direncanakan. `fresco doctor` memberi tahu Anda sedang berada di kasus yang mana.

### Apakah Fresco berjalan di KDE Plasma, COSMIC, Hyprland, atau Sway?

Ya, di keempatnya. COSMIC, Hyprland, dan Sway memutar video hidup lewat layer-shell. KDE Plasma 6 (X11 dan Wayland) memakai plugin wallpaper milik Fresco sendiri di dalam plasmashell, yang menjaga ikon desktop tetap terlihat tetapi memutar lewat Qt, bukan mpv: tanpa suara, tanpa pangkas atau transisi, dan memerlukan `qml6-module-qtmultimedia` untuk video. Opsi Metode pengisian (Penuhi, Sesuaikan, atau Regangkan) juga berlaku di sana.

### Apakah Fresco berjalan di Linux Mint, Xfce, Cinnamon, atau MATE?

Ya. Cinnamon, Xfce, dan MATE di X11 memakai renderer tertanam, dan ikon desktop tetap terlihat di atas wallpaper pada Xfce dan MATE; di Xfce Anda bisa mengeklik, memilih, dan menyeretnya seperti biasa. Cinnamon di Wayland berfungsi mulai Cinnamon 6.8, yang menambahkan layer-shell.

### Bagaimana cara berganti wallpaper dengan pintasan keyboard?

Kaitkan `fresco next`, `fresco prev`, atau `fresco random` ke sebuah tombol di pengaturan compositor atau desktop Anda — perintah-perintah itu mengganti wallpaper tanpa membuka aplikasi (`random` tidak pernah memilih wallpaper yang sedang diputar). Perintah-perintah itu berpindah di pustaka Anda sesuai urutan yang ditampilkan aplikasi, atau hanya di folder yang sedang Anda buka di sana, dan menjalankan layanan Fresco jika belum berjalan. Jika jendela Fresco sedang terbuka saat itu, jendela tersebut mengikuti wallpaper baru alih-alih memasang kembali yang lama. Di dalam aplikasi, Ctrl+K membuka palet perintah. Contohnya di Sway:

```
bindsym $mod+Right  exec fresco next
bindsym $mod+Left   exec fresco prev
bindsym $mod+Return exec fresco random
```

### Bisakah memakai wallpaper berbeda di setiap monitor?

Bisa. Fresco mendukung wallpaper per layar, dan saat video yang sama dipakai di beberapa monitor, pemutarannya dijaga tetap sinkron.

### Apakah mendukung GIF dan slideshow gambar?

Ya — GIF animasi, gambar statis, slideshow gambar dengan transisi (memudar ke hitam, Ken Burns, perbesar, dan buram), dan daftar putar beberapa video, selain berkas video.

### Bisakah Fresco memakai folder wallpaper yang sudah ada tanpa mengimpor?

Bisa. Gunakan **Tautkan folder** untuk mengarahkan Fresco ke folder tempat Anda sudah menyimpan wallpaper. Gambar, GIF, dan video di dalamnya muncul di pustaka Anda di tempatnya berada, tanpa diimpor, dan pustaka tetap sinkron saat Anda menambah dan menghapus berkas di folder itu. Subfolder bersifat opsional, dan favorit serta suntingan Anda diingat per berkas. **Tambah** tetap mengimpor berkas satuan seperti sebelumnya.

### Bisakah Fresco menangani ribuan wallpaper?

Bisa. Pustaka hanya membuat kartu untuk wallpaper di dekat posisi gulir Anda, bukan semuanya sekaligus, sehingga koleksi besar tidak membebani aplikasi. Pada pustaka uji berisi 4.000 wallpaper (setengah gambar, setengah video), pustaka muncul dalam sekitar 2 detik, bukan 9 sampai 13 detik, dan aplikasi memakai sekitar 325 MB memori, bukan 1,6 GB. Angka-angka itu berasal dari build pengembangan yang berjalan di bawah beban pada satu mesin, jadi anggaplah sebagai gambaran, bukan janji.

### Bisakah memakai video YouTube sebagai wallpaper?

Tidak dengan menempelkan tautannya. **Dari tautan** menerima pin Pinterest dan tautan langsung ke berkas video atau gambar (mp4, webm, mkv, mov, gif, jpg, png, webp); tautan lain ditolak dengan pesan. Tambahkan berkas video yang sudah Anda miliki dengan **Tambah**, atau tempel tautan langsung ke berkasnya.

### Apakah Fresco gratis dan open source?

Ya. Fresco gratis dan open source di bawah GPL-3.0-or-later. Tanpa iklan, tanpa akun, dan tanpa paket berbayar.

### Apakah Fresco mengumpulkan data saya?

Tidak ada yang dikirim sampai Anda menjawab dialog persetujuan satu kali. Anda memilih antara penghitungan anonim harian (ID instalasi acak, negara, versi aplikasi) atau itu ditambah detail opsional; mengatur `telemetry = false` di `~/.config/fresco/config.toml` berarti tidak ada yang dikirim sama sekali. Lihat [Privasi dan ketentuan penggunaan](#privasi-dan-ketentuan-penggunaan) untuk setiap kolom datanya.

### Apa bedanya Fresco dengan mpvpaper?

`mpvpaper` adalah pembungkus mpv berbasis command line yang memutar satu video sebagai latar belakang layer-shell Wayland — tanpa GUI, tanpa pustaka, tanpa dukungan X11. Fresco adalah aplikasi desktop GTK4 yang dibangun di sekitarnya: pustaka wallpaper, editor pangkas/putar, penjadwalan, sinkronisasi multi-monitor, widget desktop, dan dukungan X11, tanpa perlu terminal. Fresco menyertakan `mpvpaper` sebagai renderer Wayland-nya, bukan menggantikannya — lihat [tabel perbandingan](#perbandingan-fresco-dengan-wallpaper-engine-hidamari-komorebi-dan-mpvpaper).

### Bisakah memakai Fresco di Kali Linux?

Bisa. Kali Linux berbasis Debian, jadi Fresco dipasang dengan cara yang sama seperti di Debian atau Ubuntu — dengan paket `.deb` atau skrip instalasi satu baris (lihat [Instalasi](#instalasi)). Desktop bawaan Kali, Xfce di X11, didukung lewat renderer X11 tertanam milik Fresco.

### Bagaimana cara menghapus beberapa wallpaper sekaligus?

Klik **Pilih** di bagian bawah (atau klik kanan sebuah wallpaper lalu pilih **Pilih…**), centang yang ingin dihapus, lalu klik **Hapus**. **Pilih semua** mengikuti pencarian yang sedang aktif, jadi Anda bisa mencari dulu lalu menghapus seluruh kelompok hasilnya. Menghapus wallpaper hanya mengeluarkannya dari pustaka Fresco — berkas sumber di disk tetap ada.

### Bagaimana cara mencopot Fresco?

Jika Anda memasang `.deb` atau memakai skrip instalasi, jalankan `sudo apt remove fresco`. Pustaka dan konfigurasi Anda tersimpan di `~/.local/share/fresco/` dan `~/.config/fresco/`; hapus kedua direktori itu untuk menghilangkan semua jejak. Untuk mengembalikan wallpaper bawaan desktop tanpa mencopot, buka Fresco dan klik **Hentikan wallpaper**.

## Widget desktop: lirik, jam, visualizer, dan sampul album

Fresco menggambar lima widget opsional di wallpaper Anda. Semuanya **nonaktif secara bawaan**; aktifkan dari menu aplikasi (Ctrl+,) → **Lanjutan…**.

| Widget | Yang Anda dapatkan |
|---|---|
| **Lirik** | Baris yang sedang dinyanyikan, selaras dengan musik. Empat preset (Minimal, Karaoke, Subtitle, Kartu), kisi penempatan sembilan titik, penggeser offset sinkronisasi, baris berikutnya yang diredupkan (opsional), serta judul lagu dan artis (opsional) |
| **Jam** | Delapan tema — Digital, Minimal, Segmen digital, Bertumpuk, Kata, Kartu, NOS, dan tampilan Layar kunci ala ponsel. Format 12 atau 24 jam, tanggal opsional, pengaturan warna dan opasitas. Detik nonaktif secara bawaan karena memerlukan 60× lebih banyak penggambaran ulang |
| **Jam dunia** | Waktu di beberapa kota sekaligus, dipilih dari daftar lengkap zona waktu, sebagai teks 12 jam biasa (misalnya `Tokyo  9:41 PM`) dengan label opsional per kota. Digambar ulang sekali per menit. Baru di 1.1.50 |
| **Visualizer audio** | Lima gaya — Batang, Cerminkan, Gelombang, Titik, Cincin — dengan pemilih warna, paduan dua warna, atau pelangi |
| **Sampul album** | Sampul lagu yang sedang diputar pada piringan hitam yang berputar; berhenti berputar saat pemutaran dijeda |

Kecuali mode opsional **Melayang di atas jendela** pada jam dunia, widget digambar ke wallpaper lewat lapisan OSD mpv dan bukan ke jendelanya sendiri, sehingga tidak pernah berada di atas jendela Anda, tidak pernah menangkap klik, dan berperilaku sama persis di X11 dan di setiap compositor layer-shell. Dengan musik diputar dan widget lirik, jam, visualizer, dan sampul album aktif, biaya yang terukur adalah **0,8% dari satu inti CPU** — hampir seluruhnya dari penangkapan audio, karena tidak ada yang digambar ulang kecuali isinya berubah. Widget juga kembali dengan sendirinya setelah komputer ditangguhkan lalu dilanjutkan.

Widget muncul di **setiap layar** secara bawaan. Untuk membatasinya di satu layar, tambahkan `monitor = "DP-1"` ke blok `[widgets]` di `config.toml` (belum ada kontrol GUI untuk ini). Widget **tidak tersedia di GNOME pada Wayland**, yang tidak punya permukaan wallpaper hidup untuk digambari Fresco — alasan yang sama dengan wallpaper yang kembali ke bingkai statis di sana.

### Bisakah menambahkan jam ke wallpaper desktop Linux?

Bisa. Aktifkan widget **Jam** untuk jam bertema yang digambar di wallpaper, atau **Jam dunia** untuk menampilkan beberapa kota sekaligus. Jam dunia berada di wallpaper seperti widget lainnya; mode opsional **Melayang di atas jendela** mengangkatnya di atas semua aplikasi dengan klik yang menembus, dan ini berfungsi di X11 dengan compositor serta di compositor layer-shell (KDE Plasma, COSMIC, Sway, Hyprland), lalu kembali ke wallpaper di sesi yang tidak bisa membuatnya melayang, seperti GNOME di Wayland.

### Bisakah menampilkan lirik lagu di desktop Linux?

Bisa. Fresco menggambar **lirik yang tersinkron waktu** di wallpaper Anda, mengikuti apa pun yang sedang diputar di sistem lewat MPRIS — browser, aplikasi musik, pemutar video. Pilih preset, penempatan, dan offset sinkronisasi di pengaturan widget Lirik (lihat tabel di atas).

### Apakah Linux punya widget desktop seperti Conky?

Punya, dan Fresco menambahkan lima widget yang tidak memerlukan panel, ekstensi, ataupun dukungan dari desktop Anda: jam, jam dunia, lirik tersinkron, visualizer audio, dan cakram sampul album yang berputar. Karena digambar di wallpaper dan bukan di jendela, widget-widget ini berfungsi di desktop yang tidak punya lapisan widget sendiri — termasuk COSMIC, Hyprland, dan Sway. Berbeda dengan Conky, Fresco **tidak punya widget pemantau sistem** (tidak ada pembacaan CPU, RAM, suhu, atau jaringan), jadi Fresco hadir berdampingan dengan Conky sebagai pendamping musik dan waktu, bukan penggantinya. GNOME di Wayland adalah satu-satunya tempat yang tidak bisa dijalankannya.

### Bisakah memasang visualizer musik di latar belakang desktop?

Bisa. Visualizer audio Fresco bereaksi terhadap apa pun yang sedang diputar sistem Anda, dalam lima gaya — Batang, Cerminkan, Gelombang, Titik, atau Cincin — dengan pemilih warna, paduan dua warna, atau pelangi. Visualizer ini nonaktif secara bawaan dan meminta persetujuan saat pertama kali diaktifkan, karena ia harus mendengarkan keluaran audio Anda; persetujuan itu juga ditegakkan saat konfigurasi dimuat, sehingga mengedit `config.toml` secara manual tidak bisa mengaktifkannya di belakang Anda. Padukan dengan widget sampul album untuk piringan hitam yang berputar dengan sampul lagu saat ini.

### Pemutar musik apa yang cocok dengan widget lirik?

Apa pun yang menerbitkan metadata MPRIS standar, tetapi keandalannya tidak sama:

| Pemutar | Berfungsi? | Catatan |
|---|---|---|
| **Firefox** | ✅ | Pilihan paling andal, dan menjadi acuan saat fitur ini diverifikasi |
| Chrome / Brave / Edge / Vivaldi / Opera | ⚠️ | Lihat di bawah |
| Spotify — **di browser** | ✅ | Melaporkan posisi pemutaran dengan benar |
| Spotify — **klien Linux native** | ⚠️ | Selalu melaporkan posisinya 0, sehingga lirik tidak bisa tetap sinkron |
| Pemutar lokal (VLC, mpv, Rhythmbox, …) | ✅ | Juga kasus yang paling mungkin memiliki berkas `.lrc` lokal |

Browser keluarga Chromium mengklaim nama MPRIS saat tab mana pun pertama kali
memutar media dan **tidak pernah melepaskannya** — bug lama di Chromium. Saat
pemutaran berakhir, judulnya hilang tetapi sesi "zombie" yang usang tertinggal
di bus, kadang masih membawa gambar sampul. Fresco mengabaikan pemutar mana pun
yang tidak menerbitkan judul lagu, sehingga sesi-sesi itu terlewati, tetapi
pelaporan Chromium sendiri masih cukup tidak konsisten sehingga Firefox adalah
browser yang sebaiknya dipakai untuk ini.

Klien Spotify native untuk Linux mengembalikan `Position: 0` dan tidak pernah
mengirim seek sejak 2018, baik di build native maupun snap. Fresco mendeteksinya
dari perilakunya — tiga pembacaan nol berjarak waktu saat sedang memutar — lalu
menjalankan jam lirik secara bebas sejak pergantian lagu, yang akan melenceng
jika Anda melompat-lompat. Spotify di browser tidak punya masalah seperti itu.

### Dari mana lirik berasal?

Berkas `.lrc` lokal lebih dulu: yang berada di samping berkas audio, atau yang
cocok di folder lirik yang Anda tunjuk untuk Fresco. Jalur ini berfungsi offline
dan paling cocok, karena itu adalah berkas yang Anda pilih sendiri.

Jika tidak ada berkas lokal — yang paling sering terjadi kalau Anda memakai
streaming — Fresco mencari lagunya di [LRCLIB](https://lrclib.net), basis data
lirik tersinkron gratis yang dikelola komunitas, lalu menyimpan jawabannya di
`~/.cache/fresco/lyrics` sehingga lagu yang sama tidak pernah diambil dua kali.
Pencarian itu mengirim judul, artis, dan album lagu ke LRCLIB; tidak ada yang
dikirim kecuali widget lirik aktif dan lagu tersebut tidak punya berkas lokal.
Fresco tidak menyimpan, memiliki, ataupun melisensikan konten lirik — entri
LRCLIB disumbangkan oleh penggunanya dan LRCLIB tidak menyatakan lisensi apa
pun atasnya, sehingga Fresco mengambilnya sesuai permintaan dan menyimpannya
per pengguna, bukan membawa basis data lirik sendiri.

Pewaktuan lirik dalam berkas `.lrc` dibuat manual dan sering sedikit meleset.
Ada penggeser offset sinkronisasi di pengaturan lirik untuk keperluan itu.

## Pemecahan masalah

Jalankan `fresco doctor` lebih dulu: perintah itu melaporkan sesi Anda, renderer mana yang dipilih Fresco dan dari mana asalnya, serta apakah solusi Xorg untuk GNOME berlaku. [docs/INSTALL.md](docs/INSTALL.md) membahas lebih banyak kasus, termasuk pemakaian CPU yang tinggi. Sertakan keluaran `fresco doctor` saat Anda membuat issue.

Masih buntu? [Laporkan bug](https://github.com/DibbayajyotiRoy/Fresco/issues/new?template=bug_report.yml) atau [minta fitur](https://github.com/DibbayajyotiRoy/Fresco/issues/new?template=feature_request.yml).

### Fresco tidak muncul di launcher Deepin setelah diinstal — kenapa?

Ini masalah dde-launchpad yang sudah diketahui: penyegaran pasca-instalasinya
tidak mengenali Fresco. Jalankan `killall dde-shell` (akan dimulai ulang
otomatis) atau logout lalu login lagi, dan entrinya akan muncul secara permanen.
Fresco sendiri terpasang dengan benar — Fresco tercantum di pengelola aplikasi
bawaan Deepin, dan ikonnya terselesaikan di setiap tema yang terpasang. Jika
masalah ini terjadi pada setiap pemasangan ulang di mesin yang pernah memakai
Fresco, jalankan `sh scripts/dde-launcher-diag.sh` — skrip ini hanya membaca dan
mencetak log yang layak dilampirkan pada issue.

### Ikon desktop saya tersembunyi saat wallpaper diputar di Deepin

Deepin 25 menggambar wallpaper dan ikon desktopnya dalam satu jendela buram,
sehingga wallpaper hidup yang terlihat adalah jendela yang ditumpuk di atas
jendela itu — tidak ada lapisan yang berada di antara keduanya. **Klik desktop
dan ikon akan kembali selama sepuluh detik**, cukup lama untuk membuka apa yang
Anda cari; setelah itu wallpaper kembali, dan satu klik lagi memberi sepuluh
detik lagi. Untuk mengubah jedanya, atur `dde_icon_peek_secs` di
`~/.config/fresco/config.toml` (`0` membuat wallpaper selalu di atas). Wallpaper
bawaan Deepin tetap tampil kapan pun Fresco mati atau dijeda.

### Mengapa latar desktop MATE saya gelap setelah crash?

Agar ikon desktop Caja tetap berada di atas wallpaper hidup, Fresco mengatur
latar desktop MATE ke warna kunci hampir hitam (`#010101`) selama berjalan dan
menyalin semua yang bukan warna itu — yaitu ikon — ke wallpaper. Latar Anda
sendiri disimpan lebih dulu dan dikembalikan saat Fresco berhenti atau
dinonaktifkan. Jika Fresco crash sebelum sempat melakukannya, desktop tetap
gelap; **jalankan Fresco lagi dan ia memulihkan latar Anda**. Pada server X
tanpa ekstensi Composite dan Damage, Fresco bekerja seperti di
[Deepin](#ikon-desktop-saya-tersembunyi-saat-wallpaper-diputar-di-deepin):
ikon tersembunyi, dan klik pada desktop memunculkannya kembali.

### Wallpaper saya hitam di Wayland (NVIDIA, COSMIC, Hyprland, Sway)

Wallpaper hitam — padahal semuanya berjalan dan tidak ada error di log — hampir
selalu berarti renderer mpvpaper yang disertakan Fresco terlalu lama untuk
pengaturan Anda. Versi sebelum 1.6 menginisialisasi EGL, melaporkan berhasil,
lalu tidak pernah menampilkan satu bingkai pun pada driver proprietary NVIDIA.

Jalankan `fresco doctor` lebih dulu. Perintah itu mencetak renderer yang
dipilih, dari mana biner itu berasal (bawaan paket vs. yang Anda pasang) dan
kira-kira versi berapa, serta memperingatkan jika versinya lebih lama daripada
perbaikannya:

```
  ✓ mpvpaper available (/usr/lib/fresco/mpvpaper-libmpv2)
      source: bundled · version: 1.4–1.6
  ⚠ mpvpaper may be too old …
```

Fresco sudah lebih memilih `mpvpaper` yang lebih baru di `PATH` Anda daripada
yang bawaan paket yang lebih lama, jadi memasang paket `mpvpaper` dari distro
Anda (atau membangun [upstream](https://github.com/GhostNaN/mpvpaper) dengan
`scripts/build-mpvpaper.sh`) biasanya sudah cukup. Untuk mengarahkan Fresco ke
biner tertentu, atur **`FRESCO_MPVPAPER`** ke path lengkapnya — ini menimpa
setiap pilihan lain:

```bash
mkdir -p ~/.config/environment.d
echo 'FRESCO_MPVPAPER=/home/YOU/.local/bin/mpvpaper' > ~/.config/environment.d/fresco.conf
```

Logout lalu login lagi (systemd membaca `environment.d` saat sesi dimulai),
kemudian pastikan dengan `fresco doctor` bahwa baris `source:` kini berbunyi
`FRESCO_MPVPAPER override`.

Demikian pula, **`FRESCO_HWDEC`** menimpa dekoder hardware mpv (misalnya `nvdec`,
`nvdec-copy`, `vaapi`, `no`) — alat diagnostik untuk pemakaian CPU yang tinggi.
Secara bawaan Fresco memilih `nvdec,vaapi,auto-safe` jika ada GPU NVIDIA, selain
itu `auto-safe` (video yang diputar memakai copy-back: `nvdec-copy,auto-copy` /
`auto-copy`).

## Privasi dan ketentuan penggunaan

**Tidak ada yang dikirim sampai Anda menjawab.** Dialog persetujuan muncul sekali saat peluncuran pertama, dan pilihannya bisa diubah kapan saja di Pengaturan.

Apa pun pilihannya, sekali sehari Fresco mencatat bahwa satu instalasi aktif, di negara mana, pada versi berapa. Itulah penghitungannya. Yang ditanyakan dialog adalah **detailnya**:

| Pilihan Anda | Yang dikirim Fresco |
| --- | --- |
| **Terima semua** | Penghitungan, ditambah distro, desktop, jenis sesi, backend video, jumlah monitor, fitur yang Anda pakai, jenis error, kota dan wilayah, serta waktu persis setiap check-in. |
| **Tolak opsional** | Hanya penghitungan: ID instalasi acak (tidak pernah diturunkan dari hardware atau nama Anda), negara, versi aplikasi, pengemasan. Check-in Anda disimpan sebagai **tanggal, bukan waktu**. |

**Kota dan waktu pemakaian yang persis bersifat opsional** — hanya dikirim jika Anda menerima semua, tidak pernah dikirim jika Anda menolak. Koordinat tidak pernah dikumpulkan di tingkat mana pun: endpoint geolokasi membuang lintang dan bujur, bukan mengembalikannya.

Angka-angka ini ada untuk menentukan distro dan desktop mana yang diuji sebelum rilis, serta di mana unduhan memerlukan mirror. Angka-angka ini tidak pernah dijual, dibagikan, atau dipakai untuk iklan, dan tidak ada vendor analitik yang terlibat.

**Tidak pernah dikumpulkan, apa pun pilihannya:** data pribadi, nama berkas, wallpaper Anda, alamat IP Anda, audio, ketikan keyboard, atau isi clipboard. Negara ditentukan dari IP Anda oleh Cloudflare di tepi jaringan, jadi hanya kode dua huruf yang sampai ke proyek ini. (Teks yang sengaja Anda tulis lalu Anda kirim — komentar masukan, atau pesan ke pengelola di bawah — adalah pesan yang Anda pilih untuk dikirim, bukan pengumpulan data.)

Untuk tidak mengirim apa pun sama sekali, atur `telemetry = false` dan `telemetry_prompted = false` di `~/.config/fresco/config.toml`.

### Bicara dengan pengelola, secara anonim

**Menu → Kirim pesan ke pengelola** membuka utas dua arah yang privat dengan orang yang membuat Fresco. Tanpa akun, tanpa alamat email, tanpa login GitHub. Anonimitasnya berlaku di kedua arah: Anda tidak pernah tahu siapa mereka selain "pengelola", dan mereka tidak pernah tahu siapa Anda — hanya pesan Anda dan, jika kotaknya Anda biarkan tercentang, ringkasan pengaturan yang ditampilkan di dialog.

Utas itu dikunci dengan tiket acak yang dibuat terpisah dari telemetri dan disimpan di berkas yang berbeda, sehingga percakapan tidak akan pernah bisa digabungkan dengan profil pemakaian. Cara kerjanya persis sama baik Anda memilih menerima semua maupun menolak opsional. Tidak ada yang tercipta sampai Anda mengirim pesan pertama.

📄 **[Ketentuan penggunaan dan kebijakan privasi lengkap →](TERMS.md)** — setiap kolom, dalam tabel, tanpa ada yang dilewatkan. Setiap baris yang mengirim sesuatu ada di [`src/telemetry.rs`](src/telemetry.rs), jadi Anda bisa memeriksanya sendiri, bukan sekadar percaya.

## Fakta proyek

| | |
|---|---|
| **Nama** | Fresco (ID aplikasi `io.github.dibbayajyotiroy.Fresco`) |
| **Apa itu** | Aplikasi wallpaper hidup dan wallpaper video untuk desktop Linux |
| **Lisensi** | [GPL-3.0-or-later](LICENSE) — gratis dan open source, tanpa iklan, tanpa akun |
| **Bahasa dan stack** | Rust, GTK4 / libadwaita, libmpv; renderer mpvpaper bawaan paket di Wayland |
| **Rilis pertama** | v0.0.1, Juni 2026 |
| **Versi terbaru** | 1.1.50 (2026-10-16) — lihat [Releases](https://github.com/DibbayajyotiRoy/fresco/releases/latest) untuk yang terbaru |
| **Paket** | `.deb` (amd64), skrip instalasi satu baris, Toko Aplikasi Deepin; bangun dari kode sumber di tempat lain |
| **Bahasa antarmuka** | Inggris ditambah 12 terjemahan: Tionghoa Sederhana, Rusia, Spanyol, Prancis, Jerman, Portugis (Brasil), Turki, Vietnam, Indonesia, Hindi, Bengali, dan Rumania |
| **Pengguna** | 3.000+ orang di 128 negara |
| **Pengelola** | Dibbayajyoti Roy |
| **Kode sumber** | <https://github.com/DibbayajyotiRoy/fresco> |
| **Situs web** | <https://fresco.dibbayajyoti.com> |

## Kontribusi dan masukan

Laporan bug, permintaan fitur, dan PR sangat diterima — buka [issue](https://github.com/DibbayajyotiRoy/fresco/issues), atau gunakan dialog masukan di dalam aplikasi. Lihat [CONTRIBUTING.md](CONTRIBUTING.md).

## Lisensi

[GPL-3.0-or-later](LICENSE) — gratis dan open source.

---

<sub>Fresco adalah aplikasi wallpaper hidup dan wallpaper video yang gratis dan open source untuk Linux (X11 dan Wayland), dengan widget lirik, jam, visualizer audio, dan sampul album yang digambar langsung di wallpaper. Fresco adalah alternatif native Wallpaper Engine untuk Linux sekaligus pendamping musik dan jam bagi alat seperti Conky. Dipakai oleh 3.000+ orang di 128 negara. Terakhir diperbarui: 2026-10-16.</sub>
