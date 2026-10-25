<div align="center">

<img src="./data/icons/hicolor/256x256/apps/io.github.dibbayajyotiroy.Fresco.png" width="112" alt="Fresco-App-Symbol – eine kostenlose Open-Source-App für Live-Hintergrundbilder unter Linux" />

# Fresco — Kostenlose App für Live-Hintergrundbilder und Video-Hintergründe unter Linux

[English](README.md) | [العربية](README.ar.md) | [বাংলা](README.bn.md) | **Deutsch** | [Español](README.es.md) | [Français](README.fr.md) | [हिन्दी](README.hi.md) | [Bahasa Indonesia](README.id.md) | [Português (Brasil)](README.pt-BR.md) | [Română](README.ro.md) | [Русский](README.ru.md) | [Türkçe](README.tr.md) | [Tiếng Việt](README.vi.md) | [简体中文](README.zh-CN.md)

**Fresco ist eine kostenlose, quelloffene Live-Hintergrundbild-App für Linux. Sie legt jedes Video, GIF oder Bild als animiertes Hintergrundbild auf Ihrem Desktop fest – unter X11 und Wayland.** Als native Wallpaper-Engine-Alternative für Linux bietet Fresco eine grafische Oberfläche, GPU-beschleunigte Wiedergabe und Desktop-Widgets.

[![Neueste Fresco-Version auf GitHub](https://img.shields.io/github/v/release/DibbayajyotiRoy/fresco?style=flat-square&label=release)](https://github.com/DibbayajyotiRoy/fresco/releases/latest)
[![Lizenz GPL-3.0-or-later](https://img.shields.io/github/license/DibbayajyotiRoy/fresco?style=flat-square)](LICENSE)
[![Status des Publish-Workflows](https://img.shields.io/github/actions/workflow/status/DibbayajyotiRoy/fresco/publish.yml?style=flat-square&label=publish)](https://github.com/DibbayajyotiRoy/fresco/actions/workflows/publish.yml)
[![GitHub-Sterne](https://img.shields.io/github/stars/DibbayajyotiRoy/fresco?style=flat-square)](https://github.com/DibbayajyotiRoy/fresco/stargazers)

[Website](https://fresco.dibbayajyoti.com) · [Installation](#installation) · [Unterstützte Umgebungen](#unterstützte-umgebungen) · [Vergleich](#fresco-im-vergleich-zu-wallpaper-engine-hidamari-komorebi-und-mpvpaper) · [FAQ](#häufige-fragen-faq) · [Widgets](#desktop-widgets-songtexte-uhren-audiovisualisierung-und-album-cover) · [Änderungsprotokoll](CHANGELOG.md) · [Issues](https://github.com/DibbayajyotiRoy/fresco/issues)

<img src="./data/screenshots/gallery.png" alt="Fresco-Bibliothek auf einem dunklen Linux-Desktop: ein durchsuchbares Raster aus Video-, Bild- und Diashow-Hintergrundbildern, ein Video ist als AKTIV markiert, und eine Statusanzeige meldet NVDEC-Hardware-Dekodierung" width="800" />

</div>

## Was ist Fresco?

Fresco macht aus einem Video, GIF, Bild, einer Diashow oder einer Wiedergabeliste das Hintergrundbild hinter Ihren Fenstern – über eine GTK4-/libadwaita-App, für die Sie kein Terminal brauchen. Die Wiedergabe läuft über mpv: Die Dekodierung übernimmt die GPU, und die CPU-Auslastung bleibt nahe am Leerlauf. Was das an der Steckdose kostet, steht unter [Leistung und Akkulaufzeit](#leistung-und-akkulaufzeit). Ihr Hintergrundbild wird bei jeder Anmeldung wiederhergestellt.

- **Wiedergabe:** Videos (mp4, webm, mkv, avi, mov), animierte GIFs, Bilder, Bild-Diashows und Video-Wiedergabelisten.
- **Desktops:** X11-Sitzungen (GNOME unter Xorg, Cinnamon, Xfce, MATE, Deepin 25) und Wayland (COSMIC, Hyprland, Sway, KDE Plasma 6, Cinnamon 6.8+). GNOME unter Wayland zeigt nur ein Standbild – siehe [Unterstützte Umgebungen](#unterstützte-umgebungen).
- **Hardware-Dekodierung:** VA-API (Intel, AMD) und NVDEC (NVIDIA), über mpv.
- **Bibliothek:** Verknüpfen Sie einen Hintergrundbild-Ordner, den Sie schon haben, statt ihn zu importieren, und halten Sie ihn synchron, wenn Dateien hinzukommen oder wegfallen. Die Bibliothek zeichnet nur, was auf dem Bildschirm zu sehen ist, sodass auch Tausende Hintergrundbilder schnell und schlank bleiben.
- **Widgets:** zeitlich synchronisierte Songtexte, Uhren (auch eine Weltzeituhr), eine Audiovisualisierung und Album-Cover, direkt ins Hintergrundbild gezeichnet. Alle standardmäßig aus.
- **Steuerung:** die App, eine Befehlspalette (Strg+K) und `fresco next` / `fresco prev` / `fresco random` für Tastenkürzel.
- **Installation:** ein `.deb` für Ubuntu, Debian, Linux Mint, Pop!_OS, elementary OS, Kali und Deepin (Einzeiler-Skript, manueller Download oder Deepin App Store); unter Fedora, Arch und anderen Distributionen aus dem Quellcode bauen – siehe [Installation](#installation).
- **Lizenz:** frei und quelloffen unter [GPL-3.0-or-later](LICENSE), ohne Werbung und ohne Konto.
- **Nutzer:** über 3.000 Menschen in 128 Ländern. Aktuelle Version: 1.1.52.

## Installation

Fresco gibt es als `.deb`-Paket für Debian-basierte Distributionen; überall sonst wird es aus dem Quellcode gebaut.

### Ubuntu, Debian, Linux Mint, Pop!_OS, elementary OS und Kali Linux

Installation mit einem Befehl – er lädt das neueste `.deb` von GitHub Releases herunter und installiert es mit apt:

```bash
curl -fsSL https://github.com/DibbayajyotiRoy/fresco/releases/latest/download/install.sh | FRESCO_SOURCE=github bash
```

Oder laden Sie das `.deb` von den [Releases](https://github.com/DibbayajyotiRoy/fresco/releases/latest) herunter und führen Sie aus:

```bash
sudo apt install ./fresco_*.deb
```

### Deepin 25

Öffnen Sie den **App Store**, suchen Sie nach **Fresco** und klicken Sie auf **Installieren**. Fresco ist im Community-App-Store von deepin veröffentlicht, Sie müssen also nichts von Hand herunterladen, und Updates kommen über den Store. Die Installation mit einem Befehl von oben funktioniert auch unter Deepin.

### Fedora, Arch Linux und andere Distributionen

Für diese Distributionen gibt es noch kein fertiges Paket, daher bauen Sie Fresco aus dem Quellcode. Sie brauchen Rust (über [rustup](https://rustup.rs)) sowie die Entwicklungsdateien von GTK4, libadwaita und libmpv:

```bash
git clone https://github.com/DibbayajyotiRoy/fresco.git && cd fresco
cargo build --release --all-features
```

Auf Wayland-Compositoren braucht Fresco zusätzlich den Renderer `mpvpaper`, den das `.deb` mitliefert und ein Quellcode-Build nicht:

```bash
# Arch Linux, CachyOS, EndeavourOS (AUR)
yay -S mpvpaper

# Fedora und alles andere: gegen Ihre System-libmpv bauen
scripts/build-mpvpaper.sh
install -Dm755 target/release/mpvpaper ~/.local/bin/mpvpaper   # muss im PATH liegen
```

Fresco ist weder im AUR noch auf Flathub zu finden – das AUR-Paket namens `fresco` ist ein fremdes Physikprogramm. Details und Fehlerbehebung stehen in [docs/INSTALL.md](docs/INSTALL.md).

Öffnen Sie nach der Installation **Fresco** über Ihren Anwendungsstarter. Wenn etwas nicht stimmt, führen Sie `fresco doctor` aus.

## So legen Sie ein Video als Hintergrundbild unter Linux fest

Installieren Sie Fresco, klicken Sie auf **Hinzufügen**, wählen Sie Ihr Video aus und klicken Sie auf **Als Hintergrundbild festlegen**. Das Video läuft hinter Ihren Fenstern und startet bei jeder Anmeldung automatisch wieder.

1. Installieren Sie Fresco (siehe oben) und öffnen Sie es über Ihren Anwendungsstarter.
2. Klicken Sie auf **Hinzufügen** und wählen Sie ein Video, GIF oder Bild aus – oder fügen Sie mit **Von Link** einen Link ein.
3. Optional können Sie es im Editor zuschneiden oder drehen.
4. Klicken Sie auf **Als Hintergrundbild festlegen** und schließen Sie die App.

Das Hintergrundbild läuft auch nach dem Schließen des Fensters weiter.

## Funktionen

- **Beliebige Medien** – Video in Endlosschleife (mp4/webm/mkv), animiertes GIF, Standbild, Bild-Diashow, Wiedergabeliste aus mehreren Videos
- **Von Link hinzufügen** – fügen Sie einen Pinterest-Pin oder eine beliebige direkte Video- oder Bild-URL ein; Fresco lädt die Datei herunter und öffnet den Zuschnitt-Editor
- **Ordner verknüpfen** – nutzen Sie einen Hintergrundbild-Ordner, den Sie schon haben (Bilder, GIFs und Videos), ohne ihn zu importieren; er bleibt synchron, wenn Sie Dateien hinzufügen und entfernen, Unterordner sind optional, und Ihre Favoriten und Bearbeitungen bleiben je Datei erhalten
- **Große Bibliotheken** – die Bibliothek zeichnet nur die Hintergrundbilder in der Nähe Ihrer Scrollposition, sodass Tausende Hintergrundbilder in etwa zwei Sekunden öffnen und einige Hundert Megabyte belegen (bei einer Testbibliothek mit 4.000 Hintergrundbildern: etwa 2 s und 325 MB, zuvor 9 bis 13 s und 1,6 GB)
- **Hardware-Dekodierung** – die GPU-Videodekodierung (VA-API / NVDEC) hält die CPU-Auslastung nahe am Leerlauf, indem sie die Arbeit auf die Video-Engine verlagert, wo sie messbar günstiger ist (nicht kostenlos – siehe [Leistung und Akkulaufzeit](#leistung-und-akkulaufzeit))
- **Energiesparen** – günstigere GPU-Skalierung für Laptops; gemessen etwa die Hälfte der GPU-Leistung (siehe [Leistung und Akkulaufzeit](#leistung-und-akkulaufzeit))
- **Mehrere Bildschirme** – ein eigenes Hintergrundbild pro Bildschirm, mit synchroner Wiedergabe, wenn dasselbe Video auf mehreren Monitoren läuft
- **Tag-Nacht-Zeitpläne** – Hintergrundbilder per Timer, in frei wählbaren Zeitfenstern oder nach Sonnenauf- und -untergang wechseln
- **Desktop-Widgets** – synchronisierte Songtexte, Uhren in mehreren Designs, eine Weltzeituhr, eine Audiovisualisierung und ein rotierendes Album-Cover, direkt ins Hintergrundbild gezeichnet, sodass nichts Ihre Fenster verdeckt (die Weltzeituhr hat einen optionalen Modus, in dem sie über den Fenstern schwebt); alle standardmäßig aus (siehe [Desktop-Widgets](#desktop-widgets-songtexte-uhren-audiovisualisierung-und-album-cover))
- **Sperrbildschirm** – zeigen Sie Ihr Hintergrundbild und Ihre Widgets (Uhr, Begrüßung, aktuelle Wiedergabe, Akku und mehr) auf Ihrem echten Sperrbildschirm statt auf dem Standard-Sperrbildschirm: Live-Video und Widgets unter COSMIC 1.9+ und KDE Plasma 6, Hintergrundbild samt Widgets über `fresco lock` unter Sway/Hyprland/niri und X11, anderswo ein Standbild. Standardmäßig aus, und Fresco bekommt Ihr Passwort nie zu sehen – siehe [docs/LOCKSCREEN.md](docs/LOCKSCREEN.md)
- **Tastatursteuerung** – `fresco next`, `fresco prev` und `fresco random` für Compositor-Tastenkürzel (ein geöffnetes Fresco-Fenster übernimmt den Wechsel, statt ihn rückgängig zu machen) sowie eine Befehlspalette (Strg+K) in der App
- **Stapelverwaltung** – mehrere Hintergrundbilder auf einmal auswählen und in einem Schritt entfernen
- **Integrierter Katalog** – stöbern Sie in der App durch kuratierte, ordnungsgemäß lizenzierte Hintergrundbilder
- **Automatische Pause im Vollbild** – pro Monitor, unter X11, wlroots-Compositoren, COSMIC und KDE Plasma (über dessen Hintergrundbild-Plugin); optional auch bei maximierten Fenstern, dazu Pause bei Akkubetrieb
- **Erweiterung für neue Browser-Tabs** – spiegelt Ihr Hintergrundbild in jedem neuen Tab (Chrome/Brave/Edge/Firefox; als entpackte Erweiterung aus [`./extension`](extension) laden)
- **Deepin-DDE-Unterstützung** – unter Deepin 25 passt Fresco den DDE-Desktop automatisch an, und ein Klick auf den Desktop blendet die Symbole jeweils zehn Sekunden lang wieder ein (siehe [Fehlerbehebung](#meine-desktop-symbole-sind-unter-deepin-verborgen-während-das-hintergrundbild-läuft))
- **Editor zum Zuschneiden und Drehen**, Ton und Lautstärke pro Hintergrundbild, Übergänge in Diashows und eine durchsuchbare Bibliothek

## Unterstützte Umgebungen

| Umgebung | Live-Hintergrundbild | Hinweise |
|---|---|---|
| X11 (GNOME, Cinnamon, XFCE, …) | ✅ | Eingebetteter Renderer. GNOME nur dort, wo die Distribution noch eine Xorg-Sitzung mitliefert (siehe den GNOME-Hinweis unten) |
| Kali Linux (Xfce, X11) | ✅ | Läuft auf Kalis Standardsitzung Xfce unter X11, mit demselben eingebetteten Renderer wie andere X11-Desktops |
| Xfce (X11) | ✅ | Desktop-Symbole bleiben über dem Hintergrundbild sichtbar, und Sie können sie anklicken, auswählen und ziehen |
| MATE (X11) | ✅ | Desktop-Symbole bleiben über dem Hintergrundbild sichtbar und anklickbar |
| Deepin 25 (DDE, X11) | ✅ | Automatische DDE-Anpassung – von der Community unter Deepin 25 Community build1 bestätigt |
| COSMIC (Wayland) | ✅ | layer-shell |
| Hyprland | ✅ | layer-shell |
| Sway | ✅ | layer-shell |
| Cinnamon 6.8 und neuer (Wayland) | ✅ | Der Compositor von Cinnamon unterstützt seit 6.8 layer-shell. Cinnamon 6.6 und älter haben kein layer-shell und zeigen unter Wayland ein Standbild |
| KDE Plasma 6 (X11 und Wayland) | ✅ | Wird über das eigene Hintergrundbild-Plugin von plasmashell gesetzt, daher bleiben Desktop-Symbole sichtbar. Für Video ist `qml6-module-qtmultimedia` nötig. Die Wiedergabe läuft über Qt, nicht über mpv: stumm, ohne Hardware-Dekodierung-Tuning, Zuschnitt oder Übergänge; eine Wiedergabeliste spielt ihre erste Datei, eine Diashow zeigt ihr erstes Bild. Die Option Anpassung (Ausfüllen, Einpassen, Strecken) wird berücksichtigt |
| GNOME unter Wayland | ⚠️ | Nur Standbild – Mutter bietet keine Oberfläche für Live-Hintergrundbilder, also weder Video noch Widgets. Live-Video braucht eine Fresco-GNOME-Erweiterung (geplant) |

Jede der oben genannten Umgebungen wird bei jedem Release in der CI headless getestet.

**Hinweis zu GNOME.** GNOME unter Wayland ist der eine große Mainstream-Desktop, auf dem Fresco nur ein Standbild zeigen kann. Der bisherige Ausweg, die Anmeldung an einer Xorg-Sitzung, verschwindet: GNOME 49 hat seine X11-Sitzung deaktiviert und GNOME 50 sie entfernt, sodass Ubuntu 25.10 und neuer (einschließlich 26.04 LTS) sowie Fedora 43 und neuer keine Xorg-Sitzung mehr mitliefern, auf die man wechseln könnte. Ubuntu 22.04 und 24.04 bieten weiterhin „Ubuntu on Xorg“. Wo es keine Xorg-Sitzung gibt, muss Live-Video auf dem Standard-GNOME-Desktop auf eine Fresco-GNOME-Erweiterung warten, die geplant ist; Fresco sagt das in der App offen, statt so zu tun, als liefe das Video. KDE Plasma, COSMIC, Hyprland, Sway und jeder X11-Desktop sind nicht betroffen.

> „Einfach zu bedienen, mit klarer Oberfläche – eine der wenigen Live-Hintergrundbild-Apps, die richtig an Deepin 25 angepasst sind, per .deb installierbar und mit hardwarebeschleunigter Wiedergabe flüssig im Betrieb.“
>
> — 柒玖 (deepin-Forum) / 柒仈玖 (GitHub), getestet unter Deepin 25 Community build1, X11-Sitzung, Intel Alder Lake-N [Intel Graphics]

Deepin 25 liefert X11 als Standardsitzung aus, und auf dieser Sitzung ist Fresco dort verifiziert. Deepins eigener Wayland-Compositor [Treeland](https://github.com/linuxdeepin/treeland) befindet sich noch in der Entwicklung, daher macht Fresco zu Deepin unter Wayland bislang keine Aussage.

## Fresco im Vergleich zu Wallpaper Engine, Hidamari, Komorebi und mpvpaper

| Werkzeug | Plattform | Wayland | X11 | Oberfläche | Spielt ab | Lizenz |
|---|---|---|---|---|---|---|
| **Fresco** | Linux | ✅ layer-shell-Compositoren und KDE Plasma 6; Standbild unter GNOME | ✅ | GTK4-App und eine `fresco`-CLI | Video, GIF, Bilder, Diashows, Wiedergabelisten | GPL-3.0-or-later |
| **Wallpaper Engine** | Windows (Steam); keine offizielle Linux-Version | – | – | Grafische App | Video-, Szenen- und Web-Hintergründe | Kostenpflichtig, Closed Source |
| **[linux-wallpaperengine](https://github.com/Almamu/linux-wallpaperengine)** | Linux; braucht die Assets aus einer Steam-Kopie von Wallpaper Engine | ✅ Compositoren mit layer-shell | ⚠️ nicht, wenn eine Desktop-Umgebung (GNOME, KDE) den Hintergrund zeichnet | Kommandozeile; in der README sind separate Community-GUIs aufgeführt | Wallpaper-Engine-Hintergründe | GPL-3.0 |
| **[Hidamari](https://github.com/jeffshee/hidamari)** | Linux, auf GNOME ausgerichtet | ⚠️ GNOME Wayland | ✅ | Grafische App | Video | GPL-3.0 |
| **[Komorebi](https://github.com/cheesecakeufo/komorebi)** | Linux | — | — | Grafische App | Bild-, Video- und Webseiten-Hintergründe | GPL-3.0 |
| **[mpvpaper](https://github.com/GhostNaN/mpvpaper)** | Linux, wlroots-basierte Compositoren | ✅ layer-shell | ❌ | Kommandozeile | Video, über mpv | GPL-3.0 |
| **xwinwrap** | Linux, nur X11 | ❌ | ✅ | Kommandozeile | Was Sie als Player anhängen (mpv, gifview) | — |
| **swww** (jetzt [awww](https://codeberg.org/LGFae/awww)) | Linux, layer-shell-Compositoren; nicht GNOME | ✅ | ❌ | Kommandozeilen-Daemon | Animierte GIFs und Bilder, kein Video | GPL-3.0 |

Fresco bündelt `mpvpaper` als Wayland-Renderer und baut damit auf diesem Projekt auf, statt mit ihm zu konkurrieren. Ein `—` bedeutet, dass die Dokumentation des Projekts es nicht klar sagt. Geprüft am 2026-10-16 anhand des öffentlichen Repositorys jedes Projekts: Der letzte Push im Komorebi-Repository stammt vom Juli 2025, und das GitHub-Repository von swww ist archiviert, weil das Projekt unter dem Namen awww zu Codeberg umgezogen ist.

## Leistung und Akkulaufzeit

柒玖 (deepin-Forum) / 柒仈玖 (GitHub) hat mit `turbostat` die Package-Leistung auf einem Intel N150 gemessen (Deepin 25, VA-API, zwei Durchläufe pro Stufe), während ein Video-Hintergrund lief:

| Video | Energiesparen | GPU-Leistung | Gesamte Package-Leistung |
|---|---|---|---|
| 1080p 60fps | Volle Qualität | 1,37 W | 6,00 W |
| 1080p 60fps | **Reduziert** (Standard) | **0,63 W** (−54 %) | **4,03 W** (−33 %) |
| 4K 60fps | Volle Qualität | 2,77 W | 7,94 W |
| 4K 60fps | **Reduziert** (Standard) | **1,60 W** (−42 %) | **5,95 W** (−25 %) |
| 4K 60fps | Minimum | 0,99 W (−65 %) | 4,97 W (−37 %) |

Energiesparen senkt den GPU-Aufwand für die Skalierung pro Bild. Es werden keine Bilder ausgelassen, und die Hardware-Dekodierung bleibt unberührt, die Wiedergabe bleibt also flüssig – der Preis ist die Bildschärfe, nicht die Bewegung. Reduziert ist die Voreinstellung; bei 4K-Quellen lohnt sich Minimum.

**So lesen Sie diese Zahlen.** Hardware-Dekodierung macht ein Live-Hintergrundbild nicht kostenlos – sie verlagert die Arbeit von der CPU auf die Video-Engine, wo sie günstiger ist, aber nicht verschwindet. „Geringe CPU-Auslastung“ allein ist eine schwache Aussage, weil die Alternative, mit der man vergleicht, die Software-Dekodierung ist, die weit schlechter abschneidet; das ehrliche Maß ist der Verbrauch des Gesamtsystems, und deshalb nennt die Tabelle die **gesamte Package-Leistung** und nicht nur die GPU-Leistung.

Zwei Einschränkungen zu den obigen Zahlen, damit sie nicht für mehr gehalten werden, als sie sind:

- Es ist die **gesamte Package-Leistung bei laufendem Hintergrundbild**, nicht der Mehrverbrauch des Hintergrundbilds. Auf demselben Rechner wurde keine Leerlauf-Basis gemessen, der Unterschied zwischen diesen Werten und einem ruhenden Desktop ist hier also nicht belegt. Die Prozentwerte vergleichen die *Energiespar-Stufen* untereinander, und dafür sind sie aussagekräftig.
- Es ist ein einziger Rechner (Intel N150, Alder Lake-N, VA-API, Deepin 25, zwei Durchläufe pro Stufe). Dedizierte GPUs, NVDEC und andere Treiber werden abweichen.

Ein Live-Hintergrundbild kostet immer mehr als ein statisches. Energiesparen, automatische Pause im Vollbild und Pause bei Akkubetrieb sollen diese Kosten begrenzen, nicht so tun, als wären sie null.

## Häufige Fragen (FAQ)

### Gibt es eine Wallpaper-Engine-Alternative für Linux?

Ja. Fresco ist eine kostenlose, quelloffene Linux-App, die Ihre eigenen Video-, GIF- und Bild-Hintergründe über eine grafische Oberfläche abspielt, mit Hardware-Dekodierung und ohne Kompatibilitätsschicht. Sie öffnet keine Wallpaper-Engine-eigenen Hintergrunddateien; wenn Sie Wallpaper Engine besitzen und diese Hintergründe unter Linux nutzen möchten, führt das separate Projekt [linux-wallpaperengine](https://github.com/Almamu/linux-wallpaperengine) sie über die Kommandozeile aus und braucht dafür die Assets aus einer Steam-Kopie von Wallpaper Engine. Siehe die [Vergleichstabelle](#fresco-im-vergleich-zu-wallpaper-engine-hidamari-komorebi-und-mpvpaper).

### Funktioniert Wallpaper Engine unter Linux?

Nicht nativ. Wallpaper Engine ist eine Windows-Anwendung ohne offizielle Linux-Version. Fresco ist eine native Linux-Alternative, die sich als `.deb` installieren lässt und keine Kompatibilitätsschicht braucht.

### Wie setze ich unter Ubuntu ein Video als animiertes Hintergrundbild?

Installieren Sie Fresco, öffnen Sie es, klicken Sie auf **Hinzufügen**, wählen Sie Ihr Video aus und klicken Sie auf **Als Hintergrundbild festlegen**. Das Video läuft als Desktop-Hintergrund und wird bei der Anmeldung wiederhergestellt. Die Standardsitzung von Ubuntu (GNOME unter Wayland) kann nur ein Standbild zeigen. Unter Ubuntu 22.04 und 24.04 können Sie sich für die vollständige Live-Wiedergabe an einer Sitzung **Ubuntu on Xorg** anmelden; Ubuntu 25.10 und neuer (einschließlich 26.04 LTS) liefern keine Xorg-Sitzung mit, dort braucht Live-Video auf dem Standard-GNOME-Desktop eine Fresco-GNOME-Erweiterung, die geplant ist. Ubuntu-Varianten mit KDE Plasma, Xfce oder MATE sind nicht betroffen.

### Wie bekomme ich ein Live-Hintergrundbild unter Ubuntu, Linux Mint, Pop!_OS, Fedora oder Arch Linux?

Installieren Sie das Fresco-`.deb` unter Ubuntu, Linux Mint und Pop!_OS, oder bauen Sie es unter Fedora und Arch Linux aus dem Quellcode. Die Befehle für jede Distribution stehen unter [Installation](#installation).

- **Ubuntu:** `.deb`. Auf dem Standard-GNOME-Desktop braucht die Live-Wiedergabe eine Xorg-Sitzung (Ubuntu 22.04 und 24.04); die Varianten mit KDE Plasma, Xfce und MATE sind nicht betroffen – siehe die Ubuntu-Antwort oben.
- **Linux Mint:** `.deb`. Cinnamon unter X11 wird unterstützt, Cinnamon unter Wayland ab Version 6.8.
- **Pop!_OS:** `.deb`. Der COSMIC-Desktop (Wayland, layer-shell) wird unterstützt.
- **Fedora:** aus dem Quellcode bauen. GNOME ab Fedora 43 gibt es nur noch unter Wayland und zeigt daher ein Standbild; KDE Plasma und X11-Desktops werden unterstützt.
- **Arch Linux:** aus dem Quellcode bauen und `mpvpaper` aus dem AUR installieren (`yay -S mpvpaper`).

Nur der `.deb`-Weg wird von Frescos CI abgedeckt.

### Welche Linux-Distributionen werden unterstützt?

Fresco liefert ein `.deb`-Paket für Debian- und Ubuntu-basierte Distributionen: Ubuntu, Pop!_OS, Linux Mint, Debian, elementary OS, Deepin 25 und Kali Linux. Andere Distributionen können aus dem Quellcode bauen – siehe [Installation](#installation) und [docs/INSTALL.md](docs/INSTALL.md).

### Verbrauchen Live-Hintergrundbilder viel CPU, GPU oder Akku?

CPU: nein. Akku: etwas – ein Live-Hintergrundbild ist nie kostenlos.

Fresco dekodiert Video auf der GPU (VA-API / NVDEC), daher bleibt die CPU-Auslastung nahe am Leerlauf. Damit verlagern sich die Kosten auf die Video-Engine, statt zu verschwinden. Auf einem Intel N150 gemessen, zog ein 1080p-Hintergrundbild bei der Standardstufe Energiesparen 0,63 W GPU-Leistung, bei 4,03 W gesamter Package-Leistung während der Wiedergabe (wie das zu lesen ist, einschließlich dessen, was es *nicht* aussagt, steht unter [Leistung und Akkulaufzeit](#leistung-und-akkulaufzeit)). Die automatische Pause im Vollbild und die Pause bei Akkubetrieb sorgen dafür, dass diese Kosten Ihren Akku nicht belasten, wenn es am meisten zählt.

### Funktioniert Fresco unter Wayland?

Ja, auf Compositoren, die das layer-shell-Protokoll implementieren – COSMIC, Hyprland und Sway – sowie auf KDE Plasma 6 (X11 und Wayland) über das eigene Hintergrundbild-Plugin von plasmashell. GNOME unter Wayland ist die Ausnahme: Mutter bietet keine Hintergrund-Oberfläche, daher zeigt Fresco dort ein Standbild und sagt das in der App. X11-Sitzungen werden vollständig unterstützt.

### Funktioniert Fresco unter GNOME?

Ja, unter X11, mit dem vollen Live-Hintergrundbild – Video, Widgets, alles. Unter **GNOME mit Wayland** bietet Mutter keine Hintergrund-Oberfläche, in die eine App zeichnen könnte, daher zeigt Fresco dort ein Standbild, und Widgets sind nicht verfügbar. Wenn Ihre Distribution noch eine Xorg-Sitzung mitliefert (Ubuntu 22.04 und 24.04 tun das), melden Sie sich ab und wählen Sie „GNOME on Xorg“ oder „Ubuntu on Xorg“ für die vollständige Live-Wiedergabe. GNOME 49 hat diese Sitzung deaktiviert und GNOME 50 sie entfernt, daher haben Ubuntu 25.10 und neuer (einschließlich 26.04 LTS) sowie Fedora 43 und neuer keine: Dort braucht Live-Video unter GNOME eine Fresco-GNOME-Erweiterung, die geplant ist. `fresco doctor` zeigt Ihnen, welcher Fall bei Ihnen vorliegt.

### Funktioniert Fresco unter KDE Plasma, COSMIC, Hyprland oder Sway?

Ja, auf allen vieren. COSMIC, Hyprland und Sway spielen Live-Video über layer-shell ab. KDE Plasma 6 (X11 und Wayland) nutzt Frescos eigenes Hintergrundbild-Plugin in plasmashell, das Desktop-Symbole sichtbar lässt, aber über Qt statt über mpv abspielt: stumm, ohne Zuschnitt oder Übergänge, und für Video wird `qml6-module-qtmultimedia` benötigt. Die Option Anpassung (Ausfüllen, Einpassen oder Strecken) gilt dort ebenfalls.

### Funktioniert Fresco unter Linux Mint, Xfce, Cinnamon oder MATE?

Ja. Cinnamon, Xfce und MATE unter X11 nutzen den eingebetteten Renderer, und unter Xfce und MATE bleiben Desktop-Symbole über dem Hintergrundbild sichtbar; unter Xfce können Sie sie wie gewohnt anklicken, auswählen und ziehen. Cinnamon unter Wayland funktioniert ab Cinnamon 6.8, das layer-shell eingeführt hat.

### Wie wechsle ich das Hintergrundbild per Tastenkürzel?

Binden Sie `fresco next`, `fresco prev` oder `fresco random` in Ihrem Compositor oder in den Desktop-Einstellungen an eine Taste – sie wechseln das Hintergrundbild, ohne die App zu öffnen (`random` wählt nie das gerade laufende). Sie gehen Ihre Bibliothek in der Reihenfolge durch, in der die App sie zeigt, oder nur den dort geöffneten Ordner, und starten den Fresco-Dienst, falls er nicht läuft. Ist das Fresco-Fenster gerade geöffnet, übernimmt es das neue Hintergrundbild, statt das alte wieder einzusetzen. In der App öffnet Strg+K eine Befehlspalette. Zum Beispiel in Sway:

```
bindsym $mod+Right  exec fresco next
bindsym $mod+Left   exec fresco prev
bindsym $mod+Return exec fresco random
```

### Kann ich auf jedem Monitor ein anderes Hintergrundbild verwenden?

Ja. Fresco unterstützt Hintergrundbilder je Bildschirm, und wenn dasselbe Video auf mehreren Monitoren läuft, bleibt die Wiedergabe synchron.

### Werden GIFs und Bild-Diashows unterstützt?

Ja – animierte GIFs, Standbilder, Bild-Diashows mit Übergängen (über Schwarz überblenden, Ken Burns, Zoom und Weichzeichnen) und Wiedergabelisten aus mehreren Videos, zusätzlich zu Videodateien.

### Kann Fresco meinen vorhandenen Hintergrundbild-Ordner nutzen, ohne ihn zu importieren?

Ja. Wählen Sie im Menü **Ordner verknüpfen…** (oder nach **Ordner hinzufügen** die Option **Diesen Ordner verknüpfen**), um Fresco auf einen Ordner zu richten, in dem Sie Ihre Hintergrundbilder ohnehin aufbewahren. Dessen Bilder, GIFs und Videos erscheinen in einem eigenen Abschnitt Ihrer Bibliothek, dort, wo sie liegen, ohne importiert zu werden, und die Bibliothek bleibt synchron, wenn Sie in diesem Ordner Dateien hinzufügen oder entfernen. **Unterordner einschließen** ist optional (bis zu vier Ebenen tief), und Ihre Favoriten und Bearbeitungen werden je Datei gemerkt. Fehlt der Ordner, etwa weil ein Laufwerk abgezogen wurde, bleibt sein Abschnitt als „(nicht verfügbar)“ markiert und kehrt von selbst zurück. **Verknüpfung aufheben** im Menü des Abschnitts entfernt den Abschnitt, ohne Dateien zu löschen. **Hinzufügen** importiert einzelne Dateien weiterhin wie bisher.

### Kommt Fresco mit Tausenden Hintergrundbildern zurecht?

Ja. Die Bibliothek baut nur die Karten der Hintergrundbilder auf, in deren Nähe Sie gerade scrollen, statt aller auf einmal, sodass eine große Sammlung die App nicht belastet. Bei einer Testbibliothek mit 4.000 Hintergrundbildern (je zur Hälfte Bilder und Videos) erschien die Bibliothek nach etwa 2 Sekunden statt nach 9 bis 13, und die App belegte etwa 325 MB Arbeitsspeicher statt 1,6 GB. Diese Werte stammen von einem Entwicklungs-Build unter Last auf einem einzigen Rechner – verstehen Sie sie als Richtwert, nicht als Zusage.

### Kann ich ein YouTube-Video als Video-Hintergrund verwenden?

Nicht durch Einfügen seines Links. **Von Link** akzeptiert Pinterest-Pins und direkte Links auf eine Video- oder Bilddatei (mp4, webm, mkv, mov, gif, jpg, png, webp); andere Links werden mit einer Meldung abgelehnt. Fügen Sie eine Videodatei, die Sie schon haben, mit **Hinzufügen** hinzu, oder fügen Sie einen direkten Dateilink ein.

### Ist Fresco kostenlos und Open Source?

Ja. Fresco ist kostenlos und quelloffen unter GPL-3.0-or-later. Es gibt keine Werbung, keine Konten und keine kostenpflichtige Stufe.

### Sammelt Fresco meine Daten?

Es wird nichts gesendet, bis Sie einen einmaligen Einwilligungsdialog beantworten. Sie wählen zwischen einer täglichen anonymen Zählung (zufällige Installations-ID, Land, App-Version) und dieser Zählung plus optionalen Details; mit `telemetry = false` in `~/.config/fresco/config.toml` wird überhaupt nichts gesendet. Jedes Feld finden Sie unter [Datenschutz und Nutzungsbedingungen](#datenschutz-und-nutzungsbedingungen).

### Was ist der Unterschied zwischen Fresco und mpvpaper?

`mpvpaper` ist ein Kommandozeilen-Wrapper um mpv, der ein einzelnes Video als Wayland-layer-shell-Hintergrund abspielt – ohne GUI, ohne Bibliothek, ohne X11-Unterstützung. Fresco ist eine GTK4-Desktop-App, die darauf aufbaut: mit Hintergrundbild-Bibliothek, Editor zum Zuschneiden und Drehen, Zeitplänen, Multi-Monitor-Synchronisierung, Desktop-Widgets und X11-Unterstützung, und ganz ohne Terminal. Fresco bündelt `mpvpaper` als Wayland-Renderer, statt es zu ersetzen – siehe die [Vergleichstabelle](#fresco-im-vergleich-zu-wallpaper-engine-hidamari-komorebi-und-mpvpaper).

### Kann ich Fresco unter Kali Linux verwenden?

Ja. Kali Linux basiert auf Debian, daher wird Fresco genauso installiert wie unter Debian oder Ubuntu – mit dem `.deb`-Paket oder dem Einzeiler-Installationsskript (siehe [Installation](#installation)). Kalis Standard-Desktop, Xfce unter X11, wird über Frescos eingebetteten X11-Renderer unterstützt.

### Wie entferne ich mehrere Hintergrundbilder auf einmal?

Klicken Sie in der Fußleiste auf **Auswählen** (oder klicken Sie mit der rechten Maustaste auf ein Hintergrundbild und wählen Sie **Auswählen…**), markieren Sie die gewünschten Einträge und klicken Sie auf **Entfernen**. **Alle auswählen** berücksichtigt die aktuelle Suche, Sie können also zuerst suchen und dann einen ganzen Stapel entfernen. Das Entfernen nimmt ein Hintergrundbild nur aus Ihrer Fresco-Bibliothek – die Quelldatei auf der Festplatte bleibt erhalten.

### Wie deinstalliere ich Fresco?

Wenn Sie das `.deb` installiert oder das Installationsskript verwendet haben, führen Sie `sudo apt remove fresco` aus. Ihre Bibliothek und Konfiguration liegen in `~/.local/share/fresco/` und `~/.config/fresco/`; löschen Sie diese Verzeichnisse, um jede Spur zu entfernen. Um das eigene Hintergrundbild Ihres Desktops ohne Deinstallation zurückzubekommen, öffnen Sie Fresco und klicken Sie auf **Hintergrundbild stoppen**.

## Desktop-Widgets: Songtexte, Uhren, Audiovisualisierung und Album-Cover

Fresco zeichnet fünf optionale Widgets auf Ihr Hintergrundbild. Alle sind **standardmäßig aus**; schalten Sie sie über das App-Menü (Strg+,) → **Erweitert…** ein.

| Widget | Was Sie bekommen |
|---|---|
| **Songtext** | Die aktuelle Zeile, im Takt der Musik. Vier Presets (Minimal, Karaoke, Untertitel, Karte), ein Platzierungsraster mit neun Positionen, ein Schieberegler für den Synchronisationsversatz, optional eine abgedunkelte nächste Zeile sowie optional Titel und Interpret |
| **Uhr** | Acht Designs – Digital, Minimal, Segmentanzeige, Gestapelt, Ausgeschrieben, Karte, NOS und ein Sperrbildschirm-Zifferblatt im Smartphone-Stil. 12- oder 24-Stunden-Format, optional mit Datum, mit Farb- und Deckkraft-Einstellungen. Sekunden sind standardmäßig aus, weil sie 60-mal so viele Neuzeichnungen kosten |
| **Weltzeituhr** | Die Uhrzeit in mehreren Städten gleichzeitig, aus der vollständigen Liste der Zeitzonen gewählt, als schlichter 12-Stunden-Text (zum Beispiel `Tokyo  9:41 PM`) mit optionaler Bezeichnung pro Stadt. Zeichnet einmal pro Minute neu. Neu in 1.1.50 |
| **Audiovisualisierung** | Fünf Stile – Balken, Spiegeln, Welle, Punkte, Ring – mit Farbwähler, Farbübergang aus zwei Farben oder Regenbogen |
| **Album-Cover** | Das Cover des aktuellen Titels auf einer rotierenden Schallplatte; sie bleibt stehen, wenn die Wiedergabe pausiert |

Mit Ausnahme des optionalen Modus **Über Fenstern schweben** der Weltzeituhr werden Widgets über die OSD-Ebene von mpv ins Hintergrundbild gemalt statt in ein eigenes Fenster, sodass sie nie über Ihren Fenstern liegen, nie einen Klick abfangen und unter X11 und auf jedem layer-shell-Compositor identisch funktionieren. Bei laufender Musik und eingeschalteten Widgets für Songtext, Uhr, Audiovisualisierung und Album-Cover lag der gemessene Aufwand bei **0,8 % eines CPU-Kerns** – fast alles davon entfällt auf die Audioaufnahme, denn nichts wird neu gezeichnet, solange sich sein Inhalt nicht geändert hat. Widgets kommen außerdem von selbst zurück, nachdem der Rechner in den Ruhezustand gegangen und wieder aufgewacht ist.

Widgets erscheinen standardmäßig auf **jedem Bildschirm**. Um sie auf einem einzigen zu behalten, fügen Sie im Block `[widgets]` der `config.toml` `monitor = "DP-1"` hinzu (dafür gibt es noch keine GUI-Einstellung). Sie sind unter **GNOME mit Wayland nicht verfügbar**, weil es dort keine Live-Hintergrund-Oberfläche gibt, in die Fresco zeichnen könnte – aus demselben Grund fallen Hintergrundbilder dort auf ein Standbild zurück.

### Kann ich meinem Linux-Desktop-Hintergrund eine Uhr hinzufügen?

Ja. Schalten Sie das Widget **Uhr** ein, um eine Uhr in einem Design ins Hintergrundbild zu zeichnen, oder die **Weltzeituhr**, um mehrere Städte gleichzeitig anzuzeigen. Die Weltzeituhr sitzt wie die anderen Widgets auf dem Hintergrundbild; ihr optionaler Modus **Über Fenstern schweben** hebt sie über jede App, wobei Klicks hindurchgehen. Das funktioniert unter X11 mit Compositor und auf layer-shell-Compositoren (KDE Plasma, COSMIC, Sway, Hyprland) und fällt auf das Hintergrundbild zurück, wo eine Sitzung sie nicht schweben lassen kann, etwa unter GNOME mit Wayland.

### Kann ich Songtexte auf meinem Linux-Desktop anzeigen?

Ja. Fresco zeichnet **zeitlich synchronisierte Songtexte** auf Ihr Hintergrundbild und folgt dabei allem, was auf Ihrem System über MPRIS abgespielt wird – Browsern, Musik-Apps, Videoplayern. Wählen Sie in den Einstellungen des Songtext-Widgets ein Preset, die Platzierung und den Synchronisationsversatz (siehe die Tabelle oben).

### Gibt es unter Linux Desktop-Widgets wie Conky?

Ja, und Fresco bringt fünf mit, die weder eine Leiste noch eine Erweiterung noch Unterstützung Ihres Desktops brauchen: eine Uhr, eine Weltzeituhr, synchronisierte Songtexte, eine Audiovisualisierung und eine rotierende Album-Cover-Scheibe. Weil sie ins Hintergrundbild statt in ein Fenster gemalt werden, funktionieren sie auch auf Desktops ohne eigene Widget-Ebene – darunter COSMIC, Hyprland und Sway. Anders als Conky hat Fresco **keine Systemmonitor-Widgets** (keine Anzeigen für CPU, RAM, Temperatur oder Netzwerk); es ergänzt Conky also als Begleiter für Musik und Uhrzeit, statt es zu ersetzen. GNOME mit Wayland ist der einzige Ort, an dem es nicht laufen kann.

### Kann ich eine Musik-Visualisierung auf meinem Desktop-Hintergrund anzeigen?

Ja. Frescos Audiovisualisierung reagiert auf alles, was Ihr System abspielt, in einem von fünf Stilen – Balken, Spiegeln, Welle, Punkte oder Ring – mit Farbwähler, Farbübergang aus zwei Farben oder Regenbogen. Sie ist standardmäßig aus und fragt beim ersten Einschalten um Einwilligung, weil sie Ihre Audioausgabe mithören muss; die Einwilligung wird auch beim Laden der Konfiguration erzwungen, sodass sie sich nicht durch Handarbeit an der `config.toml` heimlich einschalten lässt. Kombinieren Sie sie mit dem Album-Cover-Widget für eine rotierende Schallplatte mit dem Cover des aktuellen Titels.

### Welche Musikplayer funktionieren mit dem Songtext-Widget?

Alles, was standardmäßige MPRIS-Metadaten veröffentlicht, aber nicht alle sind gleich zuverlässig:

| Player | Funktioniert? | Hinweise |
|---|---|---|
| **Firefox** | ✅ | Die zuverlässige Wahl, und das, womit die Funktion verifiziert wurde |
| Chrome / Brave / Edge / Vivaldi / Opera | ⚠️ | Siehe unten |
| Spotify – **im Browser** | ✅ | Meldet die Wiedergabeposition korrekt |
| Spotify – **nativer Linux-Client** | ⚠️ | Meldet seine Position für immer als 0, daher können Songtexte nicht synchron bleiben |
| Lokale Player (VLC, mpv, Rhythmbox, …) | ✅ | Zugleich der Fall, in dem am ehesten eine lokale `.lrc`-Datei existiert |

Browser der Chromium-Familie belegen einen MPRIS-Namen, sobald ein Tab zum ersten
Mal Medien abspielt, und **geben ihn nie wieder frei** – ein altbekannter
Chromium-Fehler. Endet die Wiedergabe, verschwindet der Titel, aber eine
veraltete „Zombie“-Sitzung bleibt auf dem Bus zurück, manchmal sogar noch mit
Artwork. Fresco ignoriert jeden Player, der keinen Titel veröffentlicht, und
überspringt damit diese Sitzungen; Chromiums eigene Meldungen bleiben aber so
uneinheitlich, dass Firefox der Browser der Wahl für diese Funktion ist.

Spotifys nativer Linux-Client liefert seit 2018 `Position: 0` und sendet nie
ein Seek-Signal, in nativen wie in Snap-Builds. Fresco erkennt das am Verhalten
– drei zeitlich auseinanderliegende Nullwerte während der Wiedergabe – und lässt
die Songtext-Uhr ab dem Titelwechsel frei laufen, was abdriftet, wenn Sie im
Titel springen. Spotify im Browser hat dieses Problem nicht.

### Woher stammen die Songtexte?

Zuerst aus einer lokalen `.lrc`-Datei: einer neben der Audiodatei oder einer
passenden in einem Songtext-Ordner, auf den Sie Fresco verweisen. Dieser Weg
funktioniert offline und ist die beste Übereinstimmung, denn es ist die Datei,
die Sie selbst gewählt haben.

Gibt es keine lokale Datei – beim Streaming meistens der Fall –, sucht Fresco
den Titel in [LRCLIB](https://lrclib.net), einer kostenlosen, von der Community
betriebenen Datenbank synchronisierter Songtexte, und speichert die Antwort
unter `~/.cache/fresco/lyrics` zwischen, sodass derselbe Song nie zweimal
abgerufen wird. Bei dieser Abfrage werden Titel, Interpret und Album an LRCLIB
gesendet; es wird nichts gesendet, solange das Songtext-Widget nicht läuft und
der Titel keine lokale Datei hat. Fresco hostet, besitzt oder lizenziert keine
Songtext-Inhalte – die Einträge von LRCLIB stammen von dessen Nutzern, und
LRCLIB nennt keine Lizenz dafür, daher ruft Fresco sie bei Bedarf ab und
speichert sie pro Nutzer zwischen, statt eine eigene Songtext-Datenbank
mitzuliefern.

Die Zeitangaben in `.lrc`-Dateien sind von Hand gemacht und oft leicht
daneben. Genau dafür gibt es in den Songtext-Einstellungen einen Schieberegler
für den Synchronisationsversatz.

## Fehlerbehebung

Führen Sie zuerst `fresco doctor` aus: Es meldet Ihre Sitzung, welchen Renderer Fresco gewählt hat und woher er stammt, und ob der GNOME-Xorg-Umweg zutrifft. [docs/INSTALL.md](docs/INSTALL.md) behandelt weitere Fälle, darunter hohe CPU-Auslastung. Fügen Sie die Ausgabe von `fresco doctor` bei, wenn Sie ein Issue melden.

Kommen Sie nicht weiter? [Fehler melden](https://github.com/DibbayajyotiRoy/Fresco/issues/new?template=bug_report.yml) oder [Funktion vorschlagen](https://github.com/DibbayajyotiRoy/Fresco/issues/new?template=feature_request.yml).

### Fresco erscheint nach der Installation nicht im Deepin-Launcher – warum?

Ein bekanntes dde-launchpad-Problem: Dessen Aktualisierung nach der Installation
erfasst Fresco nicht. Führen Sie `killall dde-shell` aus (es startet automatisch
neu) oder melden Sie sich ab und wieder an, dann erscheint der Eintrag dauerhaft.
Fresco selbst wird korrekt installiert – es steht in Deepins eigener
Anwendungsverwaltung, und sein Symbol wird in jedem installierten Theme gefunden.
Tritt es bei jeder Neuinstallation auf einem Rechner auf, auf dem Fresco schon
einmal war, führen Sie `sh scripts/dde-launcher-diag.sh` aus – das Skript
arbeitet nur lesend und erzeugt ein Protokoll, das sich als Anhang für ein Issue
eignet.

### Meine Desktop-Symbole sind unter Deepin verborgen, während das Hintergrundbild läuft

Deepin 25 zeichnet sein Hintergrundbild und seine Desktop-Symbole in ein einziges
deckendes Fenster, daher ist ein sichtbares Live-Hintergrundbild eines, das über
diesem Fenster liegt – es gibt keine Ebene, die zwischen beiden sitzt. **Klicken
Sie auf den Desktop, und die Symbole kommen zehn Sekunden lang zurück**, lange
genug, um zu öffnen, was Sie wollten; danach kehrt das Hintergrundbild zurück,
und ein weiterer Klick verschafft weitere zehn Sekunden. Um die Dauer zu ändern,
setzen Sie `dde_icon_peek_secs` in `~/.config/fresco/config.toml` (`0` hält das
Hintergrundbild immer oben). Deepins eigenes Hintergrundbild erscheint weiterhin,
sobald Fresco aus oder pausiert ist.

### Warum sieht mein MATE-Hintergrund nach einem Absturz dunkel aus?

Damit die Desktop-Symbole von Caja über dem Live-Hintergrundbild bleiben, setzt
Fresco den Desktop-Hintergrund von MATE während der Laufzeit auf eine fast
schwarze Schlüsselfarbe (`#010101`) und kopiert alles, was nicht diese Farbe hat
– die Symbole – auf das Hintergrundbild. Ihr eigener Hintergrund wird zuvor
gesichert und wiederhergestellt, wenn Fresco stoppt oder deaktiviert wird.
Stürzt Fresco ab, bevor es das tun kann, bleibt der Desktop dunkel; **starten Sie
Fresco einfach neu, dann stellt es Ihren Hintergrund wieder her**. Auf einem
X-Server ohne die Erweiterungen Composite und Damage arbeitet Fresco stattdessen
wie unter [Deepin](#meine-desktop-symbole-sind-unter-deepin-verborgen-während-das-hintergrundbild-läuft):
Symbole verborgen, ein Klick auf den Desktop bringt sie zurück.

### Mein Hintergrundbild ist unter Wayland schwarz (NVIDIA, COSMIC, Hyprland, Sway)

Ein schwarzes Hintergrundbild – bei dem sonst alles funktioniert und das
Protokoll keine Fehler zeigt – bedeutet fast immer, dass der mitgelieferte
mpvpaper-Renderer von Fresco für Ihr System zu alt ist. Versionen vor 1.6
initialisieren EGL, melden Erfolg und zeigen dann auf dem proprietären
NVIDIA-Treiber nie ein Bild an.

Führen Sie zuerst `fresco doctor` aus. Es zeigt den gewählten Renderer, woher
diese Binärdatei stammt (mitgeliefert oder von Ihnen installiert) und ungefähr
welche Version sie hat, und warnt, wenn diese Version älter als die Korrektur
ist:

```
  ✓ mpvpaper available (/usr/lib/fresco/mpvpaper-libmpv2)
      source: bundled · version: 1.4–1.6
  ⚠ mpvpaper may be too old …
```

Fresco bevorzugt ohnehin ein neueres `mpvpaper` in Ihrem `PATH` gegenüber einem
älteren mitgelieferten; meist genügt es also, das `mpvpaper`-Paket Ihrer
Distribution zu installieren (oder [Upstream](https://github.com/GhostNaN/mpvpaper)
mit `scripts/build-mpvpaper.sh` zu bauen). Um Fresco auf eine bestimmte
Binärdatei zu verweisen, setzen Sie **`FRESCO_MPVPAPER`** auf deren vollständigen
Pfad – das überschreibt jede andere Wahl:

```bash
mkdir -p ~/.config/environment.d
echo 'FRESCO_MPVPAPER=/home/YOU/.local/bin/mpvpaper' > ~/.config/environment.d/fresco.conf
```

Melden Sie sich ab und wieder an (systemd liest `environment.d` beim Start der
Sitzung) und prüfen Sie dann mit `fresco doctor`, dass die Zeile `source:` jetzt
`FRESCO_MPVPAPER override` lautet.

Ebenso überschreibt **`FRESCO_HWDEC`** den Hardware-Decoder von mpv (z. B. `nvdec`,
`nvdec-copy`, `vaapi`, `no`) – eine Diagnosehilfe bei hoher CPU-Auslastung. Standardmäßig
wählt Fresco `nvdec,vaapi,auto-safe`, wenn eine NVIDIA-GPU vorhanden ist, sonst
`auto-safe` (gedrehtes Video nutzt Copy-back: `nvdec-copy,auto-copy` / `auto-copy`).

## Datenschutz und Nutzungsbedingungen

**Es wird nichts gesendet, bis Sie antworten.** Ein Einwilligungsdialog fragt einmal beim ersten Start, und die Wahl lässt sich jederzeit in den Einstellungen ändern.

In beiden Fällen hält Fresco einmal am Tag fest, dass eine Installation aktiv war, in welchem Land und mit welcher Version. Das ist die Zählung. Wonach der Dialog fragt, sind die **Details**:

| Ihre Wahl | Was Fresco sendet |
| --- | --- |
| **Alles akzeptieren** | Die Zählung sowie Distribution, Desktop, Sitzungstyp, Video-Backend, Anzahl der Monitore, welche Funktionen Sie nutzen, Fehlerarten, Stadt und Region und die genaue Uhrzeit jedes Check-ins. |
| **Optionales ablehnen** | Nur die Zählung: eine zufällige Installations-ID (nie aus Ihrer Hardware oder Ihrem Namen abgeleitet), Land, App-Version, Paketierung. Ihr Check-in wird als **Datum, nicht als Uhrzeit** gespeichert. |

**Stadt und genaue Nutzungszeit sind optional** – sie werden nur gesendet, wenn Sie alles akzeptieren, und nie, wenn Sie ablehnen. Koordinaten werden auf keiner Stufe erhoben: Der Geolokalisierungs-Endpunkt verwirft Breiten- und Längengrad, statt sie zurückzugeben.

Diese Zahlen dienen dazu, zu entscheiden, welche Distributionen und Desktops vor einem Release getestet werden und wo Downloads einen Mirror brauchen. Sie werden nie verkauft, weitergegeben oder für Werbung genutzt, und es ist kein Analyse-Anbieter beteiligt.

**In keinem Fall erhoben:** personenbezogene Daten, Dateinamen, Ihre Hintergrundbilder, Ihre IP-Adresse, Audio, Tastenanschläge oder Zwischenablage-Inhalte. Das Land wird von Cloudflare am Netzwerkrand aus Ihrer IP-Adresse ermittelt, sodass nur ein zweibuchstabiger Code dieses Projekt erreicht. (Text, den Sie bewusst schreiben und absenden – ein Feedback-Kommentar oder eine Nachricht an den Maintainer weiter unten –, ist eine Nachricht, die Sie senden wollten, keine Datenerhebung.)

Um gar nichts zu senden, setzen Sie `telemetry = false` und `telemetry_prompted = false` in `~/.config/fresco/config.toml`.

### Anonym mit dem Maintainer sprechen

**Menü → Nachricht an den Maintainer** öffnet einen privaten Austausch in beide Richtungen mit der Person, die Fresco entwickelt. Kein Konto, keine E-Mail-Adresse, kein GitHub-Login. Er ist in beide Richtungen anonym: Sie erfahren nicht mehr über die Person als „der Maintainer“, und die Person erfährt nicht, wer Sie sind – nur Ihre Nachrichten und, wenn Sie das Kästchen angekreuzt lassen, die im Dialog angezeigte Zusammenfassung Ihres Systems.

Der Verlauf wird über ein zufälliges Ticket zugeordnet, das getrennt von der Telemetrie erzeugt und in einer anderen Datei gespeichert wird, sodass sich ein Gespräch nie mit einem Nutzungsprofil verknüpfen lässt. Es funktioniert genau gleich, ob Sie alles akzeptiert oder Optionales abgelehnt haben. Bis zur ersten Nachricht existiert nichts.

📄 **[Vollständige Nutzungsbedingungen und Datenschutzerklärung →](TERMS.md)** – jedes Feld in einer Tabelle, nichts ausgelassen. Jede Zeile, die irgendetwas sendet, steht in [`src/telemetry.rs`](src/telemetry.rs), sodass Sie es nachprüfen können, statt zu vertrauen.

## Projektdaten

| | |
|---|---|
| **Name** | Fresco (Anwendungs-ID `io.github.dibbayajyotiroy.Fresco`) |
| **Was es ist** | Eine App für Live- und Video-Hintergrundbilder auf dem Linux-Desktop |
| **Lizenz** | [GPL-3.0-or-later](LICENSE) – frei und quelloffen, keine Werbung, kein Konto |
| **Sprache und Stack** | Rust, GTK4 / libadwaita, libmpv; ein mitgelieferter mpvpaper-Renderer unter Wayland |
| **Erste Version** | v0.0.1, Juni 2026 |
| **Aktuelle Version** | 1.1.52 (2026-10-25) – die neueste steht auf [Releases](https://github.com/DibbayajyotiRoy/fresco/releases/latest) |
| **Pakete** | `.deb` (amd64), Einzeiler-Installationsskript, Deepin App Store; anderswo Quellcode-Build |
| **Oberflächensprachen** | Englisch plus 13 Übersetzungen: Vereinfachtes Chinesisch, Russisch, Spanisch, Französisch, Deutsch, Portugiesisch (Brasilien), Türkisch, Vietnamesisch, Indonesisch, Hindi, Bengalisch, Rumänisch und Arabisch |
| **Nutzer** | Über 3.000 Menschen in 128 Ländern |
| **Maintainer** | Dibbayajyoti Roy |
| **Quellcode** | <https://github.com/DibbayajyotiRoy/fresco> |
| **Website** | <https://fresco.dibbayajyoti.com> |

## Mitwirken und Feedback

Fehlerberichte, Funktionswünsche und PRs sind willkommen – eröffnen Sie ein [Issue](https://github.com/DibbayajyotiRoy/fresco/issues) oder nutzen Sie den Feedback-Dialog in der App. Siehe [CONTRIBUTING.md](CONTRIBUTING.md).

## Lizenz

[GPL-3.0-or-later](LICENSE) – frei und quelloffen.

---

<sub>Fresco ist eine kostenlose, quelloffene App für Live-Hintergrundbilder und Video-Hintergründe unter Linux (X11 und Wayland), mit Widgets für Songtexte, Uhr, Audiovisualisierung und Album-Cover, die ins Hintergrundbild gezeichnet werden. Es ist eine native Linux-Alternative zu Wallpaper Engine und ein Begleiter für Musik und Uhrzeit neben Werkzeugen wie Conky. Genutzt von über 3.000 Menschen in 128 Ländern. Zuletzt aktualisiert: 2026-10-25.</sub>
