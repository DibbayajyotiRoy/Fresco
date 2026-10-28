<div align="center">

<img src="./data/icons/hicolor/256x256/apps/io.github.dibbayajyotiroy.Fresco.png" width="112" alt="Fresco uygulama simgesi — Linux için ücretsiz ve açık kaynaklı bir canlı duvar kağıdı uygulaması" />

# Fresco — Linux için Ücretsiz Canlı ve Video Duvar Kağıdı Uygulaması

[English](README.md) | [العربية](README.ar.md) | [বাংলা](README.bn.md) | [Deutsch](README.de.md) | [Español](README.es.md) | [Français](README.fr.md) | [हिन्दी](README.hi.md) | [Bahasa Indonesia](README.id.md) | [Português (Brasil)](README.pt-BR.md) | [Română](README.ro.md) | [Русский](README.ru.md) | **Türkçe** | [Tiếng Việt](README.vi.md) | [简体中文](README.zh-CN.md)

**Fresco, Linux için ücretsiz ve açık kaynaklı bir canlı duvar kağıdı (hareketli duvar kağıdı) uygulamasıdır; herhangi bir videoyu, GIF’i veya görseli X11 ve Wayland üzerinde hareketli masaüstü duvar kağıdı olarak ayarlar.** Linux için yerel bir Wallpaper Engine alternatifidir; grafik arayüzlü bir uygulama, GPU hızlandırmalı oynatma ve masaüstü widget’ları sunar.

[![GitHub’daki en son Fresco sürümü](https://img.shields.io/github/v/release/DibbayajyotiRoy/fresco?style=flat-square&label=release)](https://github.com/DibbayajyotiRoy/fresco/releases/latest)
[![GPL-3.0-or-later lisansı](https://img.shields.io/github/license/DibbayajyotiRoy/fresco?style=flat-square)](LICENSE)
[![Yayın iş akışı durumu](https://img.shields.io/github/actions/workflow/status/DibbayajyotiRoy/fresco/publish.yml?style=flat-square&label=publish)](https://github.com/DibbayajyotiRoy/fresco/actions/workflows/publish.yml)
[![GitHub yıldızları](https://img.shields.io/github/stars/DibbayajyotiRoy/fresco?style=flat-square)](https://github.com/DibbayajyotiRoy/fresco/stargazers)

[Web sitesi](https://fresco.dibbayajyoti.com) · [Kurulum](#kurulum) · [Desteklenen ortamlar](#desteklenen-ortamlar) · [Karşılaştırma](#fresco-wallpaper-engine-hidamari-komorebi-ve-mpvpaper-karşılaştırması) · [SSS](#sık-sorulan-sorular-sss) · [Widget’lar](#masaüstü-widgetları-şarkı-sözleri-saatler-görselleştirici-ve-albüm-kapağı) · [Değişiklik günlüğü](CHANGELOG.md) · [Issues](https://github.com/DibbayajyotiRoy/fresco/issues)

<img src="./data/screenshots/gallery.png" alt="Koyu temalı bir Linux masaüstünde Fresco’nun duvar kağıdı kitaplığı: video, görsel ve slayt gösterisi duvar kağıtlarından oluşan aranabilir bir ızgara; bir video etkin olarak işaretli ve NVDEC donanım çözmeyi gösteren bir durum rozeti var" width="800" />

</div>

## Fresco nedir?

Fresco; bir videoyu, GIF’i, görseli, slayt gösterisini veya çalma listesini pencerelerinizin arkasındaki duvar kağıdına dönüştürür. Bunu terminal gerektirmeyen bir GTK4 / libadwaita uygulamasıyla yapar. Oynatma mpv üzerinden çalışır; yani çözme işi GPU’da yapılır ve işlemci kullanımı boşta durumuna yakın kalır — prizdeki karşılığı için [Performans](#performans-ve-pil-ömrü) bölümüne bakın. Duvar kağıdınız her oturum açışta geri yüklenir.

- **Desteklenen medya:** videolar (mp4, webm, mkv, avi, mov), animasyonlu GIF’ler, görseller, görsel slayt gösterileri ve video çalma listeleri.
- **Masaüstleri:** X11 oturumları (Xorg üzerinde GNOME, Cinnamon, Xfce, MATE, Deepin 25) ve Wayland (COSMIC, Hyprland, Sway, KDE Plasma 6, Cinnamon 6.8+). Wayland üzerindeki GNOME yalnızca durağan bir kare gösterir — bkz. [Desteklenen ortamlar](#desteklenen-ortamlar).
- **Donanım çözme:** mpv üzerinden VA-API (Intel, AMD) ve NVDEC (NVIDIA).
- **Kitaplık:** zaten sahip olduğunuz bir duvar kağıdı klasörünü içe aktarmak yerine bağlayın ve dosyalar gelip gittikçe eşitlenmiş tutun. Kitaplık yalnızca ekranda görünenleri çizer, bu yüzden binlerce duvar kağıdı hızlı ve hafif kalır.
- **Widget’lar:** müzikle eşzamanlı şarkı sözleri, saatler (bir dünya saati dahil), bir ses görselleştirici ve albüm kapağı; duvar kağıdının içine çizilir. Hepsi varsayılan olarak kapalıdır.
- **Kontrol:** uygulamanın kendisi, Ctrl+K komut paleti ve klavye kısayolları için `fresco next` / `fresco prev` / `fresco random`.
- **Kurulum:** Ubuntu, Debian, Linux Mint, Pop!_OS, elementary OS, Kali ve Deepin için bir `.deb` paketi (tek satırlık betik, elle indirme veya Deepin Uygulama Mağazası); Fedora, Arch ve diğerlerinde kaynaktan derleme — bkz. [Kurulum](#kurulum).
- **Lisans:** [GPL-3.0-or-later](LICENSE) altında ücretsiz ve açık kaynaklı; reklam yok, hesap yok.
- **Kullanıcılar:** 128 ülkede 3.000’den fazla kişi. Son sürüm: 1.1.53.

## Kurulum

Fresco, Debian ailesi dağıtımlar için `.deb` olarak paketlenir; diğer her yerde kaynaktan derlenir.

### Ubuntu, Debian, Linux Mint, Pop!_OS, elementary OS ve Kali Linux

Tek satırlık kurulum — en son `.deb` dosyasını GitHub Releases’ten indirir ve apt ile kurar:

```bash
curl -fsSL https://github.com/DibbayajyotiRoy/fresco/releases/latest/download/install.sh | FRESCO_SOURCE=github bash
```

Ya da `.deb` dosyasını [Releases](https://github.com/DibbayajyotiRoy/fresco/releases/latest) sayfasından indirin ve şunu çalıştırın:

```bash
sudo apt install ./fresco_*.deb
```

### Deepin 25

**Uygulama Mağazası**’nı açın, **Fresco**’yu aratın ve **Kur**’a tıklayın. Fresco, deepin topluluk Uygulama Mağazası’nda yayımlanmıştır; yani elle indirecek bir şey yoktur ve güncellemeler mağaza üzerinden gelir. Yukarıdaki tek satırlık kurulum Deepin’de de çalışır.

### Fedora, Arch Linux ve diğer dağıtımlar

Bunlar için henüz hazır paket yok, bu yüzden kaynaktan derleyin. Rust’a ([rustup](https://rustup.rs) ile) ve GTK4, libadwaita ile libmpv geliştirme başlıklarına ihtiyacınız var:

```bash
git clone https://github.com/DibbayajyotiRoy/fresco.git && cd fresco
cargo build --release --all-features
```

Wayland kompozitörlerinde Fresco ayrıca `mpvpaper` oluşturucusuna da ihtiyaç duyar; `.deb` bunu içinde taşır, kaynaktan derleme taşımaz:

```bash
# Arch Linux, CachyOS, EndeavourOS (AUR)
yay -S mpvpaper

# Fedora ve diğer her şey: sisteminizin libmpv’sine karşı derleyin
scripts/build-mpvpaper.sh
install -Dm755 target/release/mpvpaper ~/.local/bin/mpvpaper   # PATH içinde olmalı
```

Fresco, AUR’da veya Flathub’da yoktur — AUR’daki `fresco` adlı paket, bununla ilgisi olmayan bir fizik programıdır. Ayrıntılar ve sorun giderme [docs/INSTALL.md](docs/INSTALL.md) dosyasındadır.

Kurulumdan sonra uygulama başlatıcınızdan **Fresco**’yu açın. Bir şey yanlış görünüyorsa `fresco doctor` komutunu çalıştırın.

## Linux’ta videoyu duvar kağıdı olarak ayarlama

Fresco’yu kurun, **Ekle**’ye tıklayın, videonuzu seçin ve **Duvar kâğıdı olarak ayarla**’ya tıklayın. Video pencerelerinizin arkasında oynar ve oturum açtığınızda otomatik olarak geri gelir.

1. Fresco’yu kurun (yukarıda) ve uygulama başlatıcınızdan açın.
2. **Ekle**’ye tıklayın ve bir video, GIF veya görsel seçin — ya da **Bağlantıdan** ile bir bağlantı yapıştırın.
3. İsterseniz düzenleyicide kırpın veya döndürün.
4. **Duvar kâğıdı olarak ayarla**’ya tıklayın, ardından uygulamayı kapatın.

Pencere kapandıktan sonra da duvar kağıdı oynamaya devam eder.

## Özellikler

- **Her tür medya** — döngüde video (mp4/webm/mkv), animasyonlu GIF, sabit görsel, görsel slayt gösterisi, çok videolu çalma listesi
- **Bağlantıdan ekleme** — bir Pinterest pini veya doğrudan bir video/görsel URL’si yapıştırın; Fresco indirir ve kırpma düzenleyicisini açar
- **Klasör bağla** — zaten sahip olduğunuz bir duvar kağıdı klasörünü (görseller, GIF’ler ve videolar) içe aktarmadan kullanın; dosya ekleyip kaldırdıkça eşitlenmiş kalır, alt klasörler isteğe bağlıdır, favorileriniz ve düzenlemeleriniz dosya başına korunur
- **Büyük kitaplıklar** — kitaplık yalnızca kaydırma konumunuza yakın duvar kağıtlarını çizer; böylece binlerce duvar kağıdı yaklaşık iki saniyede açılır ve birkaç yüz megabayt kullanır (4.000 duvar kağıtlı bir test kitaplığında: yaklaşık 2 sn ve 325 MB; önceden 9 ila 13 sn ve 1,6 GB)
- **Donanım çözme** — GPU ile video çözme (VA-API / NVDEC), işi ölçülebilir biçimde daha ucuz olduğu video motoruna kaydırarak işlemci kullanımını boşta durumuna yakın tutar (bedava değil — bkz. [Performans](#performans-ve-pil-ömrü))
- **Güç tasarrufu** — dizüstü bilgisayarlar için daha ucuz GPU ölçekleme; GPU gücünü kabaca yarıya indirdiği ölçüldü (bkz. [Performans](#performans-ve-pil-ömrü))
- **Çoklu monitör** — her ekranda farklı bir duvar kağıdı, aynı video birden fazla monitörde kullanıldığında eşzamanlı oynatma
- **Gündüz ve gece zamanlamaları** — duvar kağıtlarını bir zamanlayıcıya, rastgele saat aralıklarına veya gün doğumu/batımına göre değiştirin
- **Masaüstü widget’ları** — eşzamanlı şarkı sözleri, temalı saatler, bir dünya saati, bir ses görselleştirici ve dönen bir albüm kapağı diski; doğrudan duvar kağıdının içine çizilir, böylece hiçbir şey pencerelerinizi örtmez (dünya saatinin pencerelerin üzerinde yüzen isteğe bağlı bir kipi vardır); hepsi varsayılan olarak kapalıdır (bkz. [Masaüstü widget’ları](#masaüstü-widgetları-şarkı-sözleri-saatler-görselleştirici-ve-albüm-kapağı))
- **Kilit ekranı** — duvar kağıdınızı ve widget’ları (saat, karşılama, çalan parça, pil ve daha fazlası) varsayılan kilit ekranı yerine gerçek kilit ekranınızda gösterin — COSMIC 1.9+ ve KDE Plasma 6’da canlı video ve widget’lar, Sway/Hyprland/niri ve X11’de `fresco lock` ile duvar kağıdı ve widget’lar, diğer yerlerde durağan bir kare. Varsayılan olarak kapalıdır ve Fresco parolanızı asla görmez — bkz. [docs/LOCKSCREEN.md](docs/LOCKSCREEN.md)
- **Klavye kontrolü** — kompozitör tuş atamaları için `fresco next`, `fresco prev` ve `fresco random` (açık bir Fresco penceresi değişikliği geri almak yerine üstlenir), uygulamanın içinde ise Ctrl+K komut paleti
- **Toplu yönetim** — birkaç duvar kağıdını aynı anda seçin ve tek adımda kaldırın
- **Yerleşik katalog** — lisansı düzgün, özenle seçilmiş duvar kağıtlarına uygulamanın içinden göz atın
- **Tam ekranda otomatik duraklatma** — monitör başına; X11’de, wlroots kompozitörlerinde, COSMIC’te ve KDE Plasma’da (kendi duvar kağıdı eklentisi üzerinden); isteğe bağlı olarak ekranı kaplayan pencereler için de, ayrıca pilde duraklatma
- **Tarayıcı yeni sekme uzantısı** — duvar kağıdınızı her yeni sekmede yansıtın (Chrome/Brave/Edge/Firefox; [`./extension`](extension) klasöründen paketlenmemiş uzantı olarak yükleyin)
- **Deepin DDE desteği** — Deepin 25’te Fresco, DDE masaüstüne otomatik uyum sağlar ve masaüstüne tıklamak, ihtiyaç duyduğunuzda simgeleri on saniyeliğine geri getirir (bkz. [Sorun giderme](#deepinde-duvar-kağıdı-oynarken-masaüstü-simgeleri-gizleniyor))
- **Kırpma ve döndürme düzenleyicisi**, duvar kağıdı başına ses/ses düzeyi, slayt gösterisi geçişleri ve aranabilir bir kitaplık

## Desteklenen ortamlar

| Ortam | Canlı duvar kağıdı | Notlar |
|---|---|---|
| X11 (GNOME, Cinnamon, XFCE, …) | ✅ | Yerleşik oluşturucu. GNOME yalnızca dağıtımın hâlâ bir Xorg oturumu sunduğu yerlerde (aşağıdaki GNOME notuna bakın) |
| Kali Linux (Xfce, X11) | ✅ | Kali’nin varsayılan X11 üzerinde Xfce oturumunda, diğer X11 masaüstleriyle aynı yerleşik oluşturucu ile çalışır |
| Xfce (X11) | ✅ | Masaüstü simgeleri duvar kağıdının üzerinde görünür kalır; üzerlerine tıklayabilir, onları seçebilir ve sürükleyebilirsiniz |
| MATE (X11) | ✅ | Masaüstü simgeleri duvar kağıdının üzerinde görünür ve tıklanabilir kalır |
| Deepin 25 (DDE, X11) | ✅ | Otomatik DDE uyarlaması — Deepin 25 Community build1’de topluluk tarafından doğrulandı |
| COSMIC (Wayland) | ✅ | layer-shell |
| Hyprland | ✅ | layer-shell |
| Sway | ✅ | layer-shell |
| Cinnamon 6.8 ve üstü (Wayland) | ✅ | Cinnamon’ın kompozitörü layer-shell desteğini 6.8’de kazandı. Cinnamon 6.6 ve öncesinde layer-shell yoktur ve Wayland’de durağan bir kare kalır |
| KDE Plasma 6 (X11 ve Wayland) | ✅ | plasmashell’in kendi duvar kağıdı eklentisiyle ayarlanır, bu yüzden masaüstü simgeleri görünür kalır. Video için `qml6-module-qtmultimedia` gerekir. Oynatmayı mpv değil Qt yapar: sessiz, hwdec ayarı, kırpma veya geçiş yok; çalma listesi ilk dosyayı oynatır, slayt gösterisi ilk kareyi gösterir. Sığdırma seçeneği (Kapla, Sığdır, Uzat) dikkate alınır |
| Wayland üzerinde GNOME | ⚠️ | Yalnızca durağan kare — Mutter canlı duvar kağıdı yüzeyi sunmaz, dolayısıyla video da widget da yoktur. Canlı video için bir Fresco GNOME uzantısı gerekir (planlandı) |

Yukarıdaki her ortam, her sürümde CI’da arayüzsüz (headless) olarak denenir.

**GNOME notu.** Wayland üzerindeki GNOME, Fresco’nun yalnızca durağan bir kare gösterebildiği tek yaygın masaüstüdür. Buna karşı eski çözüm olan Xorg oturumuna girmek ortadan kalkıyor: GNOME 49 X11 oturumunu devre dışı bıraktı, GNOME 50 ise kaldırdı; bu yüzden Ubuntu 25.10 ve sonrası (26.04 LTS dahil) ile Fedora 43 ve sonrası, geçilecek bir Xorg oturumu sunmuyor. Ubuntu 22.04 ve 24.04 hâlâ “Ubuntu on Xorg” sunuyor. Xorg oturumunun olmadığı yerlerde, varsayılan GNOME masaüstünde canlı video planlanmış olan bir Fresco GNOME uzantısını beklemek zorunda; Fresco, videonun oynadığını varsaymak yerine bunu uygulamada söyler. KDE Plasma, COSMIC, Hyprland, Sway ve tüm X11 masaüstleri bundan etkilenmez.

> “Kullanımı kolay, temiz bir arayüzü var — Deepin 25 için düzgünce uyarlanmış az sayıdaki canlı duvar kağıdı uygulamasından biri; .deb ile kurulabiliyor ve donanım hızlandırmalı oynatmayla sorunsuz çalışıyor.”
>
> — 柒玖 (deepin forumu) / 柒仈玖 (GitHub), Deepin 25 Community build1, X11 oturumu, Intel Alder Lake-N [Intel Graphics] üzerinde test edildi

Deepin 25 varsayılan oturum olarak X11 ile gelir ve Fresco orada bu oturumda doğrulanmıştır. Deepin’in kendi Wayland kompozitörü [Treeland](https://github.com/linuxdeepin/treeland) hâlâ geliştirilmektedir; bu yüzden Fresco şimdilik Wayland üzerindeki Deepin hakkında bir iddiada bulunmaz.

## Fresco, Wallpaper Engine, Hidamari, Komorebi ve mpvpaper karşılaştırması

| Araç | Platform | Wayland | X11 | Arayüz | Oynattığı içerik | Lisans |
|---|---|---|---|---|---|---|
| **Fresco** | Linux | ✅ layer-shell kompozitörleri ve KDE Plasma 6; GNOME’da durağan kare | ✅ | GTK4 uygulaması, ayrıca bir `fresco` CLI | Video, GIF, görseller, slayt gösterileri, çalma listeleri | GPL-3.0-or-later |
| **Wallpaper Engine** | Windows (Steam); resmî Linux sürümü yok | yok | yok | Grafik uygulama | Video, sahne ve web duvar kağıtları | Ücretli, kapalı kaynak |
| **[linux-wallpaperengine](https://github.com/Almamu/linux-wallpaperengine)** | Linux; Wallpaper Engine’in Steam kopyasındaki varlıklara ihtiyaç duyar | ✅ layer-shell destekli kompozitörler | ⚠️ arka planı bir masaüstü ortamı (GNOME, KDE) çiziyorsa çalışmaz | Komut satırı; README’si ayrı topluluk GUI’lerini listeler | Wallpaper Engine duvar kağıtları | GPL-3.0 |
| **[Hidamari](https://github.com/jeffshee/hidamari)** | Linux, GNOME odaklı | ⚠️ Wayland üzerinde GNOME | ✅ | Grafik uygulama | Video | GPL-3.0 |
| **[Komorebi](https://github.com/cheesecakeufo/komorebi)** | Linux | — | — | Grafik uygulama | Görsel, video ve web sayfası duvar kağıtları | GPL-3.0 |
| **[mpvpaper](https://github.com/GhostNaN/mpvpaper)** | Linux, wlroots tabanlı kompozitörler | ✅ layer-shell | ❌ | Komut satırı | Video, mpv üzerinden | GPL-3.0 |
| **xwinwrap** | Linux, yalnızca X11 | ❌ | ✅ | Komut satırı | Bağladığınız her oynatıcı (mpv, gifview) | — |
| **swww** (şimdi [awww](https://codeberg.org/LGFae/awww)) | Linux, layer-shell kompozitörleri; GNOME değil | ✅ | ❌ | Komut satırı arka plan servisi | Animasyonlu GIF’ler ve görseller, video değil | GPL-3.0 |

Fresco, Wayland oluşturucusu olarak `mpvpaper`’ı bünyesinde taşır; yani o projeyle rekabet etmez, onun üzerine kurulur. Bir `—`, projenin kendi belgelerinin bunu açıkça söylemediği anlamına gelir. Her projenin herkese açık deposuna karşı 2026-10-16 tarihinde kontrol edildi: Komorebi deposuna yapılan son push Temmuz 2025’teydi ve swww’nin GitHub deposu, proje Codeberg’e awww adıyla taşındığı için arşivlenmiştir.

## Performans ve pil ömrü

柒玖 (deepin forumu) / 柒仈玖 (GitHub), bir video duvar kağıdı oynarken Intel N150 üzerinde (Deepin 25, VA-API, seviye başına iki çalıştırma) paket gücünü `turbostat` ile ölçtü:

| Video | Güç tasarrufu | GPU gücü | Toplam paket gücü |
|---|---|---|---|
| 1080p 60fps | Tam kalite | 1,37 W | 6,00 W |
| 1080p 60fps | **Azaltılmış** (varsayılan) | **0,63 W** (−%54) | **4,03 W** (−%33) |
| 4K 60fps | Tam kalite | 2,77 W | 7,94 W |
| 4K 60fps | **Azaltılmış** (varsayılan) | **1,60 W** (−%42) | **5,95 W** (−%25) |
| 4K 60fps | En düşük | 0,99 W (−%65) | 4,97 W (−%37) |

Güç tasarrufu, kare başına GPU ölçekleme maliyetini düşürür. Hiçbir kare atlanmaz ve donanım çözmeye dokunulmaz; bu yüzden oynatma akıcı kalır — bedeli hareket değil, görüntü keskinliğidir. Azaltılmış varsayılandır; 4K kaynaklar için En düşük seçmeye değer.

**Bu sayılar nasıl okunmalı.** Donanım çözme, canlı duvar kağıdını bedava yapmaz — işi işlemciden video motoruna kaydırır; orada daha ucuzdur ama yok olmaz. Tek başına “düşük işlemci kullanımı” zayıf bir iddiadır, çünkü karşılaştırıldığı alternatif çok daha kötü olan yazılımla çözmedir; dürüst ölçü tüm sistemin çektiği güçtür, tablonun yalnızca GPU gücünü değil **toplam paket gücünü** de vermesinin nedeni budur.

Yukarıdaki rakamlar için, olduğundan fazlası olarak okunmasınlar diye iki not:

- Bunlar duvar kağıdının marjinal maliyeti değil, **bir duvar kağıdı oynarken toplam paket gücüdür**. Aynı makinede boşta temel değeri kaydedilmedi; dolayısıyla bu sayılarla boşta bir masaüstü arasındaki fark burada belirlenmiş değildir. Yüzdeler güç tasarrufu *seviyelerini* birbiriyle karşılaştırır ve yalnızca bunun için geçerlidir.
- Bunlar tek bir makineye aittir (Intel N150, Alder Lake-N, VA-API, Deepin 25, seviye başına iki çalıştırma). Ayrık GPU’lar, NVDEC ve diğer sürücüler farklı sonuç verecektir.

Canlı duvar kağıdı her zaman sabit bir duvar kağıdından daha pahalıdır. Güç tasarrufu, tam ekranda otomatik duraklatma ve pilde duraklatma bu maliyeti sınırlamak için vardır; sıfırmış gibi davranmak için değil.

## Sık sorulan sorular (SSS)

### Linux için Wallpaper Engine alternatifi var mı?

Evet. Fresco, kendi video, GIF ve görsel duvar kağıtlarınızı grafik arayüzle, donanım çözmeyle ve uyumluluk katmanı olmadan oynatan ücretsiz, açık kaynaklı bir Linux uygulamasıdır. Wallpaper Engine’in kendi duvar kağıdı dosyalarını açmaz; Wallpaper Engine’e sahipseniz ve o duvar kağıtlarını Linux’ta istiyorsanız, ayrı proje [linux-wallpaperengine](https://github.com/Almamu/linux-wallpaperengine) bunları komut satırından çalıştırır ve Wallpaper Engine’in Steam kopyasındaki varlıklara ihtiyaç duyar. Bkz. [karşılaştırma tablosu](#fresco-wallpaper-engine-hidamari-komorebi-ve-mpvpaper-karşılaştırması).

### Wallpaper Engine Linux’ta çalışır mı?

Yerel olarak hayır. Wallpaper Engine, resmî Linux sürümü olmayan bir Windows uygulamasıdır. Fresco ise `.deb` olarak kurulan ve uyumluluk katmanına ihtiyaç duymayan yerel bir Linux alternatifidir.

### Ubuntu’da video nasıl duvar kağıdı yapılır?

Fresco’yu kurun, açın, **Ekle**’ye tıklayın, videonuzu seçin ve **Duvar kâğıdı olarak ayarla**’ya tıklayın. Video masaüstü arka planınız olarak oynar ve oturum açıldığında geri yüklenir. Ubuntu’nun varsayılan oturumu olan Wayland üzerinde GNOME yalnızca durağan bir kare gösterebilir. Ubuntu 22.04 ve 24.04’te tam canlı oynatma için bir **Ubuntu on Xorg** oturumuna girebilirsiniz; Ubuntu 25.10 ve sonrası (26.04 LTS dahil) Xorg oturumu sunmaz, bu yüzden orada varsayılan GNOME masaüstünde canlı video, planlanmış olan bir Fresco GNOME uzantısını gerektirir. KDE Plasma, Xfce veya MATE kullanan Ubuntu çeşitleri bundan etkilenmez.

### Ubuntu, Linux Mint, Pop!_OS, Fedora veya Arch Linux’ta canlı duvar kağıdı nasıl kullanılır?

Ubuntu, Linux Mint ve Pop!_OS’ta Fresco’nun `.deb` paketini kurun; Fedora ve Arch Linux’ta kaynaktan derleyin. Her biri için komutlar [Kurulum](#kurulum) bölümündedir.

- **Ubuntu:** `.deb`. Varsayılan GNOME masaüstünde canlı oynatma için bir Xorg oturumu gerekir (Ubuntu 22.04 ve 24.04); KDE Plasma, Xfce ve MATE çeşitleri etkilenmez — yukarıdaki Ubuntu yanıtına bakın.
- **Linux Mint:** `.deb`. X11 üzerinde Cinnamon desteklenir, Wayland üzerinde Cinnamon ise 6.8 sürümünden itibaren.
- **Pop!_OS:** `.deb`. COSMIC masaüstü (Wayland layer-shell) desteklenir.
- **Fedora:** kaynaktan derleyin. Fedora 43 ve sonrasının GNOME’u yalnızca Wayland olduğundan durağan bir kare gösterir; KDE Plasma ve X11 masaüstleri desteklenir.
- **Arch Linux:** kaynaktan derleyin ve `mpvpaper`’ı AUR’dan kurun (`yay -S mpvpaper`).

Yalnızca `.deb` yolu Fresco’nun CI’ı tarafından kapsanır.

### Hangi Linux dağıtımları destekleniyor?

Fresco, Debian ve Ubuntu tabanlı dağıtımlar için bir `.deb` paketi sunar: Ubuntu, Pop!_OS, Linux Mint, Debian, elementary OS, Deepin 25 ve Kali Linux. Diğer dağıtımlar kaynaktan derleyebilir — bkz. [Kurulum](#kurulum) ve [docs/INSTALL.md](docs/INSTALL.md).

### Canlı duvar kağıtları çok fazla işlemci, GPU veya pil harcar mı?

İşlemci, hayır. Pil, bir miktar — canlı duvar kağıdı hiçbir zaman bedava değildir.

Fresco videoyu GPU’da çözer (VA-API / NVDEC), bu yüzden işlemci kullanımı boşta durumuna yakın kalır. Bu, maliyeti ortadan kaldırmak yerine video motoruna kaydırır. Varsayılan Güç tasarrufu seviyesinde bir Intel N150 üzerinde ölçüldüğünde, 1080p bir duvar kağıdı oynarken GPU gücü 0,63 W, toplam paket gücü 4,03 W çekti (bunun nasıl okunacağı ve neyi *söylemediği* için [Performans](#performans-ve-pil-ömrü) bölümüne bakın). Tam ekranda otomatik duraklatma ve pilde duraklatma, bu maliyeti en çok önem taşıdığı anda pilinizden uzak tutmak için vardır.

### Fresco Wayland’de çalışır mı?

Evet; layer-shell protokolünü uygulayan kompozitörlerde — COSMIC, Hyprland ve Sway — ve plasmashell’in kendi duvar kağıdı eklentisi üzerinden KDE Plasma 6’da (X11 ve Wayland). Wayland üzerindeki GNOME istisnadır: Mutter duvar kağıdı yüzeyi sunmaz, bu yüzden Fresco orada durağan bir kare gösterir ve bunu uygulamada söyler. X11 oturumları tam olarak desteklenir.

### Fresco GNOME’da çalışır mı?

X11’de evet; tam canlı duvar kağıdıyla — video, widget’lar, her şey. **Wayland altındaki GNOME**’da Mutter, hiçbir uygulamanın çizim yapabileceği bir duvar kağıdı yüzeyi sunmaz; bu yüzden Fresco orada durağan bir kare gösterir ve widget’lar kullanılamaz. Dağıtımınız hâlâ bir Xorg oturumu sunuyorsa (Ubuntu 22.04 ve 24.04 sunuyor), oturumu kapatıp tam canlı oynatma için “GNOME on Xorg” veya “Ubuntu on Xorg” seçeneğini seçin. GNOME 49 bu oturumu devre dışı bıraktı, GNOME 50 ise kaldırdı; bu yüzden Ubuntu 25.10 ve sonrası (26.04 LTS dahil) ile Fedora 43 ve sonrasında yok: bunlarda GNOME üzerinde canlı video, planlanmış olan bir Fresco GNOME uzantısını gerektirir. `fresco doctor` hangi durumda olduğunuzu söyler.

### Fresco KDE Plasma, COSMIC, Hyprland veya Sway’de çalışır mı?

Evet, dördünde de. COSMIC, Hyprland ve Sway canlı videoyu layer-shell üzerinden oynatır. KDE Plasma 6 (X11 ve Wayland), plasmashell içinde Fresco’nun kendi duvar kağıdı eklentisini kullanır; bu, masaüstü simgelerini görünür tutar ama oynatmayı mpv yerine Qt ile yapar: sessiz, kırpma veya geçiş olmadan ve video için `qml6-module-qtmultimedia` gerekir. Sığdırma seçeneği (Kapla, Sığdır veya Uzat) orada da geçerlidir.

### Fresco, Linux Mint, Xfce, Cinnamon ve MATE masaüstlerinde çalışır mı?

Evet. X11 üzerindeki Cinnamon, Xfce ve MATE yerleşik oluşturucuyu kullanır; Xfce ve MATE’te masaüstü simgeleri duvar kağıdının üzerinde görünür kalır; Xfce’te bunlara her zamanki gibi tıklayabilir, onları seçebilir ve sürükleyebilirsiniz. Wayland üzerindeki Cinnamon, layer-shell ekleyen Cinnamon 6.8’den itibaren çalışır.

### Klavye kısayoluyla duvar kağıdını nasıl değiştiririm?

`fresco next`, `fresco prev` veya `fresco random` komutunu kompozitörünüzün ya da masaüstünüzün ayarlarında bir tuşa atayın — uygulamayı açmadan duvar kağıdını değiştirirler (`random` hiçbir zaman zaten oynatılanı seçmez). Kitaplığınızda uygulamanın gösterdiği sırayla ya da yalnızca orada açık olan klasörde ilerlerler ve çalışmıyorsa Fresco servisini başlatırlar. Fresco penceresi o sırada açıksa, eski duvar kağıdını geri koymak yerine yeni duvar kağıdını yakalar. Uygulamanın içinde Ctrl+K bir komut paleti açar. Örneğin Sway’de:

```
bindsym $mod+Right  exec fresco next
bindsym $mod+Left   exec fresco prev
bindsym $mod+Return exec fresco random
```

### Her monitörde farklı bir duvar kağıdı kullanabilir miyim?

Evet. Fresco ekran başına duvar kağıdını destekler ve aynı video birkaç monitörde kullanıldığında oynatma eşzamanlı tutulur.

### GIF ve görsel slayt gösterilerini destekliyor mu?

Evet — video dosyalarına ek olarak animasyonlu GIF’ler, sabit görseller, geçişli (siyaha solarak geçiş, Ken Burns, yakınlaştırma ve bulanıklık) görsel slayt gösterileri ve çok videolu çalma listeleri.

### Fresco, içe aktarmadan mevcut duvar kağıdı klasörümü kullanabilir mi?

Evet. Fresco’yu duvar kağıtlarınızı zaten sakladığınız bir klasöre yöneltmek için menüden **Klasör bağla…** seçeneğini (ya da **Klasör ekle**’den sonra **Bu klasörü bağla**’yı) kullanın. Klasördeki görseller, GIF’ler ve videolar içe aktarılmadan, bulundukları yerde kitaplığınızda ayrı bir bölümde görünür; o klasöre dosya ekleyip kaldırdıkça kitaplık eşitlenmiş kalır. **Alt klasörleri dahil et** isteğe bağlıdır (dört düzeye kadar iner), favorileriniz ve düzenlemeleriniz dosya başına hatırlanır. Klasör ortadan kalkarsa, örneğin sürücü çıkarılırsa, bölümü “(kullanılamıyor)” olarak işaretli kalır ve kendiliğinden geri gelir. **Klasör bağlantısını kaldır**, bölümün menüsünde, hiçbir dosyayı silmeden bölümü kaldırır. **Ekle** tek tek dosyaları eskisi gibi içe aktarmaya devam eder.

### Fresco binlerce duvar kağıdını kaldırabilir mi?

Evet. Kitaplık, tüm duvar kağıtlarının kartlarını bir anda oluşturmak yerine yalnızca kaydırdığınız yerin yakınındakilerin kartlarını oluşturur; bu yüzden büyük bir koleksiyon uygulamayı ağırlaştırmaz. 4.000 duvar kağıtlı (yarısı görsel, yarısı video) bir test kitaplığında kitaplık 9 ila 13 saniye yerine yaklaşık 2 saniyede göründü ve uygulama 1,6 GB yerine yaklaşık 325 MB bellek kullandı. Bu rakamlar tek bir makinede, yük altındaki bir geliştirme sürümünden geliyor; söz olarak değil, yol gösterici olarak okuyun.

### YouTube videosunu duvar kağıdı olarak kullanabilir miyim?

Bağlantısını yapıştırarak hayır. **Bağlantıdan**, Pinterest pinlerini ve bir video ya da görsel dosyasına giden doğrudan bağlantıları (mp4, webm, mkv, mov, gif, jpg, png, webp) kabul eder; diğer bağlantılar bir mesajla reddedilir. Elinizde hazır olan bir video dosyasını **Ekle** ile ekleyin ya da doğrudan bir dosya bağlantısı yapıştırın.

### Fresco ücretsiz ve açık kaynaklı mı?

Evet. Fresco, GPL-3.0-or-later altında ücretsiz ve açık kaynaklıdır. Reklam yok, hesap yok, ücretli katman yok.

### Fresco verilerimi topluyor mu?

Bir kerelik onay penceresini yanıtlayana kadar hiçbir şey gönderilmez. Günlük anonim bir sayım (rastgele kurulum kimliği, ülke, uygulama sürümü) ile bu sayım artı isteğe bağlı ayrıntılar arasında seçim yaparsınız; `~/.config/fresco/config.toml` içinde `telemetry = false` ayarlamak hiçbir şey göndermez. Her alan için [Gizlilik ve kullanım koşulları](#gizlilik-ve-kullanım-koşulları) bölümüne bakın.

### Fresco ile mpvpaper arasındaki fark nedir?

`mpvpaper`, tek bir videoyu Wayland layer-shell arka planı olarak oynatan, komut satırında çalışan bir mpv sarmalayıcısıdır — GUI yok, kitaplık yok, X11 desteği yok. Fresco ise onun etrafında kurulmuş bir GTK4 masaüstü uygulamasıdır: duvar kağıdı kitaplığı, kırpma/döndürme düzenleyicisi, zamanlama, çoklu monitör eşzamanlaması, masaüstü widget’ları ve X11 desteği; terminal gerekmez. Fresco, `mpvpaper`’ı yerine geçmek yerine Wayland oluşturucusu olarak bünyesinde taşır — bkz. [karşılaştırma tablosu](#fresco-wallpaper-engine-hidamari-komorebi-ve-mpvpaper-karşılaştırması).

### Fresco’yu Kali Linux’ta kullanabilir miyim?

Evet. Kali Linux Debian tabanlıdır; bu yüzden Fresco, Debian veya Ubuntu’daki gibi kurulur — `.deb` paketi ya da tek satırlık kurulum betiğiyle ([Kurulum](#kurulum) bölümüne bakın). Kali’nin varsayılan masaüstü olan X11 üzerinde Xfce, Fresco’nun yerleşik X11 oluşturucusu sayesinde desteklenir.

### Birden fazla duvar kağıdını aynı anda nasıl kaldırırım?

Alt çubuktaki **Seç**’e tıklayın (veya bir duvar kağıdına sağ tıklayıp **Seç…** seçin), istediklerinizi işaretleyin ve **Kaldır**’a tıklayın. **Tümünü seç** geçerli aramaya uyar; böylece önce arama yapıp ardından bir toplu grubu temizleyebilirsiniz. Bir duvar kağıdını kaldırmak onu Fresco kitaplığınızdan çıkarır — diskteki kaynak dosya korunur.

### Fresco nasıl kaldırılır?

`.deb` paketini kurduysanız veya kurulum betiğini kullandıysanız `sudo apt remove fresco` komutunu çalıştırın. Kitaplığınız ve yapılandırmanız `~/.local/share/fresco/` ile `~/.config/fresco/` içinde durur; her izi silmek için bu dizinleri silin. Kaldırmadan masaüstünüzün kendi duvar kağıdını geri almak için Fresco’yu açın ve **Duvar kâğıdını durdur**’a tıklayın.

## Masaüstü widget’ları: şarkı sözleri, saatler, görselleştirici ve albüm kapağı

Fresco, duvar kağıdınızın üzerine beş isteğe bağlı widget çizer. Hepsi **varsayılan olarak kapalıdır**; uygulama menüsünden (Ctrl+,) → **Gelişmiş…** ile açın.

| Widget | Ne elde edersiniz |
|---|---|
| **Şarkı sözleri** | Müzikle birlikte akan geçerli satır. Dört hazır ayar (Minimal, Karaoke, Altyazı, Kart), dokuz noktalı bir yerleşim ızgarası, bir senkronizasyon kayması kaydırıcısı, isteğe bağlı olarak soluk sonraki satır ve isteğe bağlı olarak parça adı ile sanatçı |
| **Saat** | Sekiz tema — Dijital, Minimal, Segment, Yığılmış, Yazıyla, Kart, NOS ve telefon tarzı bir Kilit ekranı yüzü. 12 veya 24 saat, isteğe bağlı tarih, renk ve opaklık denetimleri. Saniyeler varsayılan olarak kapalıdır, çünkü 60× fazla yeniden çizime mal olur |
| **Dünya saati** | Birden fazla şehirdeki saat aynı anda; tam saat dilimi listesinden seçilir, düz 12 saatlik metin olarak (örneğin `Tokyo  9:41 PM`) ve şehir başına isteğe bağlı bir etiketle. Dakikada bir yeniden çizer. 1.1.50’de yeni |
| **Ses görselleştirici** | Beş stil — Çubuklar, Ayna, Dalga, Noktalar, Halka — renk seçici, iki renkli geçiş veya gökkuşağı ile |
| **Albüm kapağı** | Geçerli parçanın kapağı dönen bir plak üzerinde; oynatma duraklatıldığında dönmeyi bırakır |

Dünya saatinin isteğe bağlı **Pencerelerin üzerinde dursun** kipi dışında, widget’lar kendi pencereleri yerine mpv’nin OSD katmanı üzerinden duvar kağıdına boyanır; bu yüzden pencerelerinizin üzerinde durmaz, tıklamayı hiç yakalamaz ve X11’de ile her layer-shell kompozitöründe aynı davranır. Müzik çalarken ve şarkı sözleri, saat, görselleştirici ve albüm kapağı widget’ları açıkken ölçülen maliyet **bir işlemci çekirdeğinin %0,8’i** oldu — neredeyse tamamı ses yakalamadan, çünkü içeriği değişmedikçe hiçbir şey yeniden çizilmez. Widget’lar ayrıca makine askıya alınıp devam ettirildikten sonra kendiliğinden geri gelir.

Widget’lar varsayılan olarak **her ekranda** görünür. Birinde tutmak için `config.toml` içindeki `[widgets]` bloğuna `monitor = "DP-1"` ekleyin (bunun için henüz GUI denetimi yok). **Wayland altındaki GNOME’da kullanılamazlar**; orada Fresco’nun çizim yapabileceği bir canlı duvar kağıdı yüzeyi yoktur — duvar kağıtlarının orada durağan kareye dönmesinin de nedeni aynıdır.

### Linux masaüstü duvar kağıdına saat ekleyebilir miyim?

Evet. Duvar kağıdına çizilen temalı bir saat için **Saat** widget’ını, birden fazla şehri aynı anda göstermek için ise **Dünya saati**’ni açın. Dünya saati, diğer widget’lar gibi duvar kağıdının üzerinde durur; isteğe bağlı **Pencerelerin üzerinde dursun** kipi onu tıklamalar içinden geçecek biçimde her uygulamanın üzerine çıkarır — bu, kompozitörlü X11’de ve layer-shell kompozitörlerinde (KDE Plasma, COSMIC, Sway, Hyprland) çalışır; oturumun onu yüzdüremediği yerlerde, örneğin Wayland üzerinde GNOME’da, duvar kağıdına geri döner.

### Linux masaüstünde şarkı sözlerini gösterebilir miyim?

Evet. Fresco, sisteminizde MPRIS üzerinden çalan her şeyi — tarayıcılar, müzik uygulamaları, video oynatıcılar — izleyerek duvar kağıdınıza **zamanla eşzamanlı şarkı sözleri** çizer. Şarkı sözleri widget’ı ayarlarında bir hazır ayar, yerleşim ve senkronizasyon kayması seçin (yukarıdaki tabloya bakın).

### Linux’ta Conky gibi masaüstü widget’ları var mı?

Evet; Fresco ise panel, uzantı ve masaüstünüzden destek gerektirmeyen beş tane ekler: bir saat, bir dünya saati, eşzamanlı şarkı sözleri, bir ses görselleştirici ve dönen bir albüm kapağı diski. Bir pencere yerine duvar kağıdına boyandıkları için kendi widget katmanı olmayan masaüstlerinde de çalışırlar — COSMIC, Hyprland ve Sway dahil. Conky’nin aksine Fresco’da **sistem izleme widget’ları yoktur** (işlemci, RAM, sıcaklık veya ağ göstergesi yok); bu yüzden Conky’nin yerini almaz, yanında bir müzik ve zaman yoldaşı olarak durur. Çalışamadığı tek yer Wayland üzerindeki GNOME’dur.

### Masaüstü arka planında müzik görselleştirici kullanabilir miyim?

Evet. Fresco’nun ses görselleştiricisi sisteminizin çaldığı her şeye tepki verir; beş stilden biriyle — Çubuklar, Ayna, Dalga, Noktalar veya Halka — renk seçici, iki renkli geçiş veya gökkuşağı ile. Varsayılan olarak kapalıdır ve ses çıkışınızı dinlemesi gerektiği için ilk açtığınızda izin ister; onay yapılandırma yüklenirken de zorunlu tutulur, yani `config.toml` dosyasını elle düzenleyerek arkanızdan açılamaz. Geçerli parçanın kapağının dönen bir plak olarak görünmesi için albüm kapağı widget’ıyla birlikte kullanın.

### Şarkı sözü widget’ı hangi müzik çalarlarla çalışır?

Standart MPRIS meta verisi yayımlayan her şeyle, ancak hepsi eşit derecede güvenilir değildir:

| Çalar | Çalışıyor mu? | Notlar |
|---|---|---|
| **Firefox** | ✅ | Güvenilir seçim; özellik bununla doğrulandı |
| Chrome / Brave / Edge / Vivaldi / Opera | ⚠️ | Aşağıya bakın |
| Spotify — **tarayıcıda** | ✅ | Oynatma konumunu doğru bildirir |
| Spotify — **yerel Linux istemcisi** | ⚠️ | Konumunu sonsuza dek 0 bildirir, bu yüzden sözler eşzamanlı kalamaz |
| Yerel çalarlar (VLC, mpv, Rhythmbox, …) | ✅ | Yerel bir `.lrc` dosyasının en olası olduğu durum da budur |

Chromium ailesi tarayıcılar, herhangi bir sekme ilk kez medya oynattığında bir
MPRIS adı talep eder ve onu **hiç bırakmaz** — uzun süredir bilinen bir Chromium
hatası. Oynatma bittiğinde başlık temizlenir ama veri yolunda eskimiş bir “zombi”
oturum kalır, bazen hâlâ bir kapak resmi taşıyarak. Fresco, parça başlığı
yayımlamayan her oynatıcıyı yok sayar; bu, o oturumları atlar, ancak Chromium’un
kendi bildirimleri bu iş için Firefox’u kullanılacak tarayıcı yapacak kadar
tutarsız kalır.

Spotify’ın yerel Linux istemcisi, 2018’den beri hem yerel hem snap sürümlerinde
`Position: 0` döndürüyor ve hiç seek göndermedi. Fresco bunu davranışsal olarak
algılar — oynatma sırasında aralıklı üç sıfır okuma — ve söz saatini parça
değişiminden itibaren serbestçe çalıştırır; parça içinde atlarsanız bu kayar.
Tarayıcıdaki Spotify’da böyle bir sorun yoktur.

### Şarkı sözleri nereden geliyor?

Önce yerel bir `.lrc` dosyasından: ses dosyasının yanında duran birinden ya da
Fresco’yu yönlendirdiğiniz bir söz klasöründeki eşleşen birinden. Bu yol çevrimdışı
çalışır ve en iyi eşleşmedir, çünkü sizin seçtiğiniz dosyadır.

Yerel dosya olmadığında — akış kullanıyorsanız çoğu zaman böyledir — Fresco parçayı
topluluk tarafından işletilen ücretsiz bir eşzamanlı söz veritabanı olan
[LRCLIB](https://lrclib.net)’de arar ve yanıtı `~/.cache/fresco/lyrics` altında
önbelleğe alır; böylece aynı şarkı iki kez getirilmez. Bu arama, parçanın başlığını,
sanatçısını ve albümünü LRCLIB’e gönderir; şarkı sözleri widget’ı açık ve parçanın
yerel dosyası yoksa dışında hiçbir şey gönderilmez. Fresco şarkı sözü içeriğini
barındırmaz, bunlara sahip değildir ve lisanslamaz — LRCLIB’in girdilerini
kullanıcıları katkı olarak ekler ve LRCLIB bunlar üzerinde bir lisans belirtmez;
bu yüzden Fresco kendi söz veritabanını dağıtmak yerine istek üzerine getirir ve
kullanıcı başına önbelleğe alır.

`.lrc` dosyalarındaki söz zamanlaması elle yapılır ve çoğu zaman biraz kayıktır.
Şarkı sözleri ayarlarında tam olarak bunun için bir senkronizasyon kayması
kaydırıcısı vardır.

## Sorun giderme

Önce `fresco doctor` komutunu çalıştırın: oturumunuzu, Fresco’nun hangi oluşturucuyu seçtiğini ve nereden geldiğini, bir de GNOME Xorg geçici çözümünün geçerli olup olmadığını bildirir. [docs/INSTALL.md](docs/INSTALL.md) yüksek işlemci kullanımı dahil daha fazla durumu kapsar. Bir issue açarken `fresco doctor` çıktısını ekleyin.

Hâlâ çözemediniz mi? [Hata bildirin](https://github.com/DibbayajyotiRoy/Fresco/issues/new?template=bug_report.yml) veya [özellik isteyin](https://github.com/DibbayajyotiRoy/Fresco/issues/new?template=feature_request.yml).

### Fresco kurulumdan sonra Deepin başlatıcısında görünmüyor — neden?

Bilinen bir dde-launchpad sorunu: kurulum sonrası yenilemesi Fresco’yu yakalamıyor.
`killall dde-shell` komutunu çalıştırın (kendiliğinden yeniden başlar) veya oturumu
kapatıp yeniden açın; giriş kalıcı olarak görünür. Fresco’nun kendisi doğru
kuruluyor — Deepin’in kendi uygulama yöneticisi tarafından listeleniyor ve simgesi
kurulu her temada çözümleniyor. Daha önce Fresco bulunmuş bir makinede her yeniden
kurulumda oluyorsa `sh scripts/dde-launcher-diag.sh` çalıştırın — salt okunurdur
ve bir issue’ya eklemeye değer bir günlük yazdırır.

### Deepin’de duvar kağıdı oynarken masaüstü simgeleri gizleniyor

Deepin 25 duvar kağıdını ve masaüstü simgelerini tek bir opak pencereye çizer;
dolayısıyla görebildiğiniz bir canlı duvar kağıdı, o pencerenin üzerine yığılmış
olandır — ikisinin arasına girecek bir katman yoktur. **Masaüstüne tıklayın,
simgeler on saniyeliğine geri gelir**; aradığınızı açmaya yeter. Bundan sonra duvar
kağıdı döner ve bir tıklama daha on saniye daha kazandırır. Gecikmeyi değiştirmek
için `~/.config/fresco/config.toml` içinde `dde_icon_peek_secs` ayarlayın (`0`
duvar kağıdını her zaman üstte tutar). Deepin’in kendi duvar kağıdı, Fresco kapalı
veya duraklatılmış olduğunda yine görünür.

### MATE masaüstü arka planım çökmeden sonra neden koyu görünüyor?

Caja’nın masaüstü simgelerini canlı duvar kağıdının üstünde tutmak için Fresco,
çalıştığı sürece MATE’in masaüstü arka planını siyaha yakın bir anahtar renge
(`#010101`) ayarlar ve bu renk olmayan her şeyi — simgeleri — duvar kağıdına
kopyalar. Kendi arka planınız önce kaydedilir ve Fresco durduğunda veya devre dışı
bırakıldığında geri konur. Fresco bunu yapamadan çökerse masaüstü koyu kalır;
**Fresco’yu yeniden başlatın, arka planınızı geri yükler**. Composite ve Damage
eklentileri olmayan bir X sunucusunda Fresco bunun yerine
[Deepin](#deepinde-duvar-kağıdı-oynarken-masaüstü-simgeleri-gizleniyor)’deki gibi
çalışır: simgeler gizlenir, masaüstüne bir tıklama onları geri getirir.

### Wayland’de duvar kağıdım siyah (NVIDIA, COSMIC, Hyprland, Sway)

Siyah bir duvar kağıdı — geri kalan her şey çalışıyor ve günlükte hata yok — hemen
her zaman Fresco’nun içinde taşıdığı mpvpaper oluşturucusunun kurulumunuz için
fazla eski olduğu anlamına gelir. 1.6’dan önceki sürümler EGL’yi başlatır, başarı
bildirir ve ardından NVIDIA’nın tescilli sürücüsünde hiçbir kare sunmaz.

Önce `fresco doctor` komutunu çalıştırın. Seçtiği oluşturucuyu, o ikilinin nereden
geldiğini (pakete dahil mi yoksa sizin kurduğunuz mu) ve kabaca hangi sürüm
olduğunu yazdırır; sürüm düzeltmeden önceyse uyarır:

```
  ✓ mpvpaper available (/usr/lib/fresco/mpvpaper-libmpv2)
      source: bundled · version: 1.4–1.6
  ⚠ mpvpaper may be too old …
```

Fresco, `PATH` üzerindeki daha yeni bir `mpvpaper`’ı pakete dahil olan daha eski
birine zaten tercih eder; bu yüzden dağıtımınızın `mpvpaper` paketini kurmak (veya
[upstream](https://github.com/GhostNaN/mpvpaper)’i `scripts/build-mpvpaper.sh` ile
derlemek) genellikle yeterlidir. Fresco’yu belirli bir ikiliye yöneltmek için
**`FRESCO_MPVPAPER`** değişkenini onun tam yoluna ayarlayın — bu, diğer tüm
seçimlerin önüne geçer:

```bash
mkdir -p ~/.config/environment.d
echo 'FRESCO_MPVPAPER=/home/YOU/.local/bin/mpvpaper' > ~/.config/environment.d/fresco.conf
```

Oturumu kapatıp yeniden açın (systemd `environment.d` dizinini oturum başında
okur), ardından `fresco doctor` ile `source:` satırının artık
`FRESCO_MPVPAPER override` dediğini doğrulayın.

Benzer şekilde **`FRESCO_HWDEC`**, mpv’nin donanım çözücüsünü geçersiz kılar (ör.
`nvdec`, `nvdec-copy`, `vaapi`, `no`) — yüksek işlemci kullanımı için bir tanı
aracıdır. Varsayılan olarak Fresco, bir NVIDIA GPU varsa `nvdec,vaapi,auto-safe`,
yoksa `auto-safe` seçer (döndürülmüş video copy-back kullanır:
`nvdec-copy,auto-copy` / `auto-copy`).

## Gizlilik ve kullanım koşulları

**Siz yanıtlayana kadar hiçbir şey gönderilmez.** Bir onay penceresi ilk açılışta bir kez sorar ve seçim Ayarlar’dan istediğiniz zaman değiştirilebilir.

Hangisini seçerseniz seçin, Fresco günde bir kez bir kurulumun etkin olduğunu, hangi ülkede ve hangi sürümde olduğunu kaydeder. Sayım budur. Pencerenin sorduğu şey ise **ayrıntıdır**:

| Seçiminiz | Fresco’nun gönderdikleri |
| --- | --- |
| **Tümünü kabul et** | Sayım; artı dağıtım, masaüstü, oturum türü, video arka ucu, monitör sayısı, hangi özellikleri kullandığınız, hata türleri, şehir ve bölge ve her kontrolün tam zamanı. |
| **İsteğe bağlıları reddet** | Yalnızca sayım: rastgele bir kurulum kimliği (donanımınızdan veya adınızdan asla türetilmez), ülke, uygulama sürümü, paketleme. Kontrolünüz **saat değil, tarih** olarak saklanır. |

**Şehir ve kullanımın tam zamanı isteğe bağlıdır** — yalnızca tümünü kabul ederseniz gönderilir, reddederseniz asla gönderilmez. Hiçbir düzeyde koordinat toplanmaz: konum bulma uç noktası enlem ve boylamı döndürmek yerine atar.

Bu sayılar, bir sürümden önce hangi dağıtımların ve masaüstlerinin test edileceğine ve indirmelerin nerede yansıya ihtiyaç duyduğuna karar vermek için vardır. Asla satılmaz, paylaşılmaz veya reklam için kullanılmaz ve işin içinde bir analitik sağlayıcı yoktur.

**Her iki durumda da asla toplanmayanlar:** kişisel veriler, dosya adları, duvar kağıtlarınız, IP adresiniz, ses, tuş vuruşları veya pano içeriği. Ülke, IP’nizden ağ kenarında Cloudflare tarafından çözümlenir; bu yüzden bu projeye yalnızca iki harfli bir kod ulaşır. (Bilerek yazıp gönder’e bastığınız metin — bir geri bildirim yorumu veya aşağıdaki geliştiriciye bir mesaj — sizin gönderilmesini seçtiğiniz bir mesajdır, toplama değildir.)

Hiçbir şey göndermemek için `~/.config/fresco/config.toml` içinde `telemetry = false` ve `telemetry_prompted = false` ayarlayın.

### Geliştiriciyle anonim olarak konuşun

**Menü → Geliştiriciye mesaj gönder**, Fresco’yu yapan kişiyle özel, çift yönlü bir konuşma açar. Hesap yok, e-posta adresi yok, GitHub girişi yok. İki yönde de anonimdir: “geliştirici”nin ötesinde kim olduğunu öğrenmezsiniz, o da kim olduğunuzu öğrenmez — yalnızca mesajlarınız ve kutuyu işaretli bırakırsanız penceredeki kurulum özeti.

Konuşma, telemetriden ayrı üretilen ve farklı bir dosyada saklanan rastgele bir bilet ile anahtarlanır; dolayısıyla bir konuşma hiçbir zaman bir kullanım profiliyle eşleştirilemez. Tümünü kabul etmiş de olsanız isteğe bağlıları reddetmiş de olsanız tam olarak aynı çalışır. İlk mesajı gönderene kadar hiçbir şey var olmaz.

📄 **[Kullanım koşullarının ve gizlilik politikasının tamamı →](TERMS.md)** — her alan, bir tabloda, hiçbir şey atlanmadan. Bir şey gönderen her satır [`src/telemetry.rs`](src/telemetry.rs) içindedir; yani güvenmek yerine kontrol edebilirsiniz.

## Proje bilgileri

| | |
|---|---|
| **Ad** | Fresco (uygulama kimliği `io.github.dibbayajyotiroy.Fresco`) |
| **Nedir** | Linux masaüstü için bir canlı ve video duvar kağıdı uygulaması |
| **Lisans** | [GPL-3.0-or-later](LICENSE) — ücretsiz ve açık kaynaklı, reklam yok, hesap yok |
| **Dil ve yığın** | Rust, GTK4 / libadwaita, libmpv; Wayland’de pakete dahil bir mpvpaper oluşturucusu |
| **İlk sürüm** | v0.0.1, Haziran 2026 |
| **Son sürüm** | 1.1.53 (2026-10-28) — en yenisi için [Releases](https://github.com/DibbayajyotiRoy/fresco/releases/latest) sayfasına bakın |
| **Paketler** | `.deb` (amd64), tek satırlık kurulum betiği, Deepin Uygulama Mağazası; diğer yerlerde kaynaktan derleme |
| **Arayüz dilleri** | İngilizce artı 13 çeviri: Basitleştirilmiş Çince, Rusça, İspanyolca, Fransızca, Almanca, Portekizce (Brezilya), Türkçe, Vietnamca, Endonezce, Hintçe, Bengalce, Romence ve Arapça |
| **Kullanıcılar** | 128 ülkede 3.000’den fazla kişi |
| **Geliştirici** | Dibbayajyoti Roy |
| **Kaynak kodu** | <https://github.com/DibbayajyotiRoy/fresco> |
| **Web sitesi** | <https://fresco.dibbayajyoti.com> |

## Katkı ve geri bildirim

Hata raporları, özellik istekleri ve PR’lar memnuniyetle karşılanır — bir [issue](https://github.com/DibbayajyotiRoy/fresco/issues) açın veya uygulama içi geri bildirim penceresini kullanın. Bkz. [CONTRIBUTING.md](CONTRIBUTING.md).

## Lisans

[GPL-3.0-or-later](LICENSE) — ücretsiz ve açık kaynaklı.

---

<sub>Fresco, Linux (X11 ve Wayland) için ücretsiz, açık kaynaklı bir canlı duvar kağıdı ve video duvar kağıdı uygulamasıdır; duvar kağıdının içine çizilen şarkı sözü, saat, ses görselleştirici ve albüm kapağı widget’ları vardır. Linux için yerel bir Wallpaper Engine alternatifidir ve Conky gibi araçlara bir müzik ve saat yoldaşıdır. 128 ülkede 3.000’den fazla kişi tarafından kullanılıyor. Son güncelleme: 2026-10-28.</sub>
