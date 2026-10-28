<div align="center">

<img src="./data/icons/hicolor/256x256/apps/io.github.dibbayajyotiroy.Fresco.png" width="112" alt="Pictograma aplicației Fresco — o aplicație gratuită și open source de fundal animat pentru Linux" />

# Fresco — aplicație gratuită de fundal animat și imagine de fundal video pentru Linux

[English](README.md) | [العربية](README.ar.md) | [বাংলা](README.bn.md) | [Deutsch](README.de.md) | [Español](README.es.md) | [Français](README.fr.md) | [हिन्दी](README.hi.md) | [Bahasa Indonesia](README.id.md) | [Português (Brasil)](README.pt-BR.md) | **Română** | [Русский](README.ru.md) | [Türkçe](README.tr.md) | [Tiếng Việt](README.vi.md) | [简体中文](README.zh-CN.md)

**Fresco este o aplicație gratuită și open source de fundal animat (imagine de fundal live) pentru Linux, care setează orice videoclip, GIF sau imagine ca imagine de fundal animată pe desktop, pe X11 și pe Wayland.** Este o alternativă nativă la Wallpaper Engine pentru Linux, cu aplicație grafică, redare accelerată de GPU și widgeturi pentru desktop.

[![Ultima versiune Fresco pe GitHub](https://img.shields.io/github/v/release/DibbayajyotiRoy/fresco?style=flat-square&label=release)](https://github.com/DibbayajyotiRoy/fresco/releases/latest)
[![Licență GPL-3.0-or-later](https://img.shields.io/github/license/DibbayajyotiRoy/fresco?style=flat-square)](LICENSE)
[![Starea fluxului de publicare](https://img.shields.io/github/actions/workflow/status/DibbayajyotiRoy/fresco/publish.yml?style=flat-square&label=publish)](https://github.com/DibbayajyotiRoy/fresco/actions/workflows/publish.yml)
[![Stele pe GitHub](https://img.shields.io/github/stars/DibbayajyotiRoy/fresco?style=flat-square)](https://github.com/DibbayajyotiRoy/fresco/stargazers)

[Site web](https://fresco.dibbayajyoti.com) · [Instalare](#instalare) · [Medii compatibile](#medii-compatibile) · [Comparație](#fresco-vs-wallpaper-engine-hidamari-komorebi-și-mpvpaper) · [Întrebări frecvente](#întrebări-frecvente-faq) · [Widgeturi](#widgeturi-pentru-desktop-versuri-ceasuri-vizualizator-și-copertă-de-album) · [Jurnal de modificări](CHANGELOG.md) · [Issues](https://github.com/DibbayajyotiRoy/fresco/issues)

<img src="./data/screenshots/gallery.png" alt="Biblioteca de imagini de fundal Fresco pe un desktop Linux întunecat: o grilă cu căutare de imagini de fundal video, imagini și succesiuni de imagini, cu un videoclip marcat ca activ și un indicator de stare care arată decodarea hardware NVDEC" width="800" />

</div>

## Ce este Fresco?

Fresco transformă un videoclip, un GIF, o imagine, o succesiune de imagini sau o listă de redare în imaginea de fundal din spatele ferestrelor, printr-o aplicație GTK4 / libadwaita care nu cere terminal. Redarea trece prin mpv, așa că decodarea se face pe GPU, iar utilizarea procesorului rămâne aproape de zero — vedeți [Performanță](#performanță-și-autonomie) pentru ce înseamnă asta la priză. Imaginea de fundal este restaurată la fiecare autentificare.

- **Redă:** videoclipuri (mp4, webm, mkv, avi, mov), GIF-uri animate, imagini, succesiuni de imagini și liste de redare video.
- **Desktopuri:** sesiuni X11 (GNOME pe Xorg, Cinnamon, Xfce, MATE, Deepin 25) și Wayland (COSMIC, Hyprland, Sway, KDE Plasma 6, Cinnamon 6.8+). GNOME pe Wayland afișează doar un cadru fix — vedeți [Medii compatibile](#medii-compatibile).
- **Decodare hardware:** VA-API (Intel, AMD) și NVDEC (NVIDIA), prin mpv.
- **Bibliotecă:** asociați un dosar cu imagini de fundal pe care le aveți deja, în loc să îl importați, și păstrați-l sincronizat pe măsură ce fișierele apar și dispar. Biblioteca desenează doar ce este pe ecran, așa că mii de imagini de fundal rămân rapide și ușoare.
- **Widgeturi:** versuri sincronizate cu muzica, ceasuri (inclusiv un ceas mondial), un vizualizator audio și coperta albumului, desenate în imaginea de fundal. Toate sunt dezactivate implicit.
- **Control:** aplicația, o paletă de comenzi cu Ctrl+K și `fresco next` / `fresco prev` / `fresco random` pentru scurtături de tastatură.
- **Instalare:** un pachet `.deb` pentru Ubuntu, Debian, Linux Mint, Pop!_OS, elementary OS, Kali și Deepin (script dintr-o linie, descărcare manuală sau magazinul de aplicații Deepin); compilare din sursă pe Fedora, Arch și altele — vedeți [Instalare](#instalare).
- **Licență:** gratuit și open source sub [GPL-3.0-or-later](LICENSE), fără reclame și fără cont.
- **Utilizatori:** peste 3.000 de persoane din 128 de țări. Ultima versiune: 1.1.53.

## Instalare

Fresco este împachetat ca `.deb` pentru distribuțiile din familia Debian și se compilează din sursă în rest.

### Ubuntu, Debian, Linux Mint, Pop!_OS, elementary OS și Kali Linux

Instalare dintr-o linie — descarcă ultimul `.deb` din GitHub Releases și îl instalează cu apt:

```bash
curl -fsSL https://github.com/DibbayajyotiRoy/fresco/releases/latest/download/install.sh | FRESCO_SOURCE=github bash
```

Sau descărcați `.deb`-ul din [Releases](https://github.com/DibbayajyotiRoy/fresco/releases/latest) și rulați:

```bash
sudo apt install ./fresco_*.deb
```

### Deepin 25

Deschideți **Magazinul de aplicații**, căutați **Fresco** și faceți clic pe **Instalare**. Fresco este publicat în magazinul de aplicații al comunității deepin, deci nu trebuie să descărcați nimic manual, iar actualizările vin prin magazin. Instalarea dintr-o linie de mai sus funcționează și pe Deepin.

### Fedora, Arch Linux și alte distribuții

Pentru acestea nu există încă un pachet precompilat, așa că compilați din sursă. Aveți nevoie de Rust (prin [rustup](https://rustup.rs)) și de antetele de dezvoltare GTK4, libadwaita și libmpv:

```bash
git clone https://github.com/DibbayajyotiRoy/fresco.git && cd fresco
cargo build --release --all-features
```

Pe compozitoarele Wayland, Fresco are nevoie și de randarea `mpvpaper`, pe care `.deb`-ul o include, iar compilarea din sursă nu:

```bash
# Arch Linux, CachyOS, EndeavourOS (AUR)
yay -S mpvpaper

# Fedora și orice altceva: compilați-l cu libmpv-ul sistemului
scripts/build-mpvpaper.sh
install -Dm755 target/release/mpvpaper ~/.local/bin/mpvpaper   # trebuie să fie în PATH
```

Fresco nu se află în AUR sau pe Flathub — pachetul AUR numit `fresco` este un program de fizică fără legătură cu el. Detalii și depanare găsiți în [docs/INSTALL.md](docs/INSTALL.md).

După instalare, deschideți **Fresco** din lansatorul de aplicații. Dacă ceva nu arată bine, rulați `fresco doctor`.

## Cum setați un videoclip ca imagine de fundal în Linux

Instalați Fresco, faceți clic pe **Adăugare**, alegeți videoclipul și faceți clic pe **Setare ca imagine de fundal**. Videoclipul rulează în spatele ferestrelor și revine automat la autentificare.

1. Instalați Fresco (mai sus) și deschideți-l din lansatorul de aplicații.
2. Faceți clic pe **Adăugare** și alegeți un videoclip, un GIF sau o imagine — sau lipiți un link cu **Din link**.
3. Opțional, decupați sau rotiți în editor.
4. Faceți clic pe **Setare ca imagine de fundal**, apoi închideți aplicația.

Imaginea de fundal continuă să ruleze și după ce fereastra se închide.

## Funcții

- **Orice tip de conținut** — videoclip în buclă (mp4/webm/mkv), GIF animat, imagine statică, succesiune de imagini, listă de redare cu mai multe videoclipuri
- **Adăugare din link** — lipiți un pin Pinterest sau orice URL direct de videoclip/imagine; Fresco îl descarcă și deschide editorul de decupare
- **Asociere dosar** — folosiți un dosar cu imagini de fundal pe care îl aveți deja (imagini, GIF-uri și videoclipuri) fără să îl importați; rămâne sincronizat pe măsură ce adăugați și eliminați fișiere, subdosarele sunt opționale, iar favoritele și modificările se păstrează pentru fiecare fișier
- **Biblioteci mari** — biblioteca desenează doar imaginile de fundal din apropierea poziției de derulare, așa că mii de imagini de fundal se deschid în aproximativ două secunde și ocupă câteva sute de megaocteți (pe o bibliotecă de test cu 4.000 de imagini de fundal: aproximativ 2 s și 325 MB, față de 9 până la 13 s și 1,6 GB)
- **Decodare hardware** — decodarea video pe GPU (VA-API / NVDEC) menține utilizarea procesorului aproape de zero, mutând munca pe motorul video, unde este măsurabil mai ieftină (nu gratuită — vedeți [Performanță](#performanță-și-autonomie))
- **Economisire energie** — scalare GPU mai ieftină pentru laptopuri; măsurată la aproximativ jumătate din consumul GPU (vedeți [Performanță](#performanță-și-autonomie))
- **Mai multe monitoare** — o imagine de fundal diferită pe fiecare ecran, cu redare sincronizată când același videoclip rulează pe mai multe monitoare
- **Programe zi și noapte** — schimbați imaginile de fundal după un cronometru, după intervale orare arbitrare sau după răsărit și apus
- **Widgeturi pentru desktop** — versuri sincronizate ale melodiilor, ceasuri tematice, un ceas mondial, un vizualizator audio și un disc rotativ cu coperta albumului, desenate chiar în imaginea de fundal, astfel încât nimic nu acoperă ferestrele (ceasul mondial are un mod opțional care plutește deasupra ferestrelor); toate sunt dezactivate implicit (vedeți [Widgeturi pentru desktop](#widgeturi-pentru-desktop-versuri-ceasuri-vizualizator-și-copertă-de-album))
- **Ecran de blocare** — afișați imaginea de fundal și widgeturile (ceas, salut, melodia curentă, baterie și altele) pe ecranul de blocare real, în locul celui implicit — video live și widgeturi pe COSMIC 1.9+ și KDE Plasma 6, imagine de fundal și widgeturi prin `fresco lock` pe Sway/Hyprland/niri și X11, un cadru fix în rest. Dezactivat implicit, iar Fresco nu vă vede niciodată parola — vedeți [docs/LOCKSCREEN.md](docs/LOCKSCREEN.md)
- **Control de la tastatură** — `fresco next`, `fresco prev` și `fresco random` pentru scurtăturile compozitorului (o fereastră Fresco deschisă preia schimbarea în loc să o anuleze) și o paletă de comenzi cu Ctrl+K în aplicație
- **Gestionare în lot** — selectați mai multe imagini de fundal deodată și eliminați-le dintr-un singur pas
- **Catalog integrat** — răsfoiți în aplicație imagini de fundal alese, cu licențe în regulă
- **Pauză automată la ecran complet** — pe fiecare monitor, pe X11, pe compozitoarele wlroots, pe COSMIC și pe KDE Plasma (prin pluginul său de imagine de fundal); opțional și pentru ferestrele maximizate, plus pauză când e pe baterie
- **Extensie pentru fila nouă din browser** — oglindiți imaginea de fundal în fiecare filă nouă (Chrome/Brave/Edge/Firefox; încărcați ca extensie despachetată din [`./extension`](extension))
- **Suport Deepin DDE** — pe Deepin 25, Fresco se adaptează automat la desktopul DDE, iar un clic pe desktop readuce pictogramele pentru zece secunde ori de câte ori aveți nevoie de ele (vedeți [Depanare](#pictogramele-de-pe-desktop-sunt-ascunse-cât-timp-rulează-fundalul-pe-deepin))
- **Editor de decupare și rotire**, sunet și volum pentru fiecare imagine de fundal, tranziții pentru succesiuni de imagini și o bibliotecă în care se poate căuta

## Medii compatibile

| Mediu | Fundal animat | Observații |
|---|---|---|
| X11 (GNOME, Cinnamon, XFCE, …) | ✅ | Randare integrată. GNOME doar acolo unde distribuția mai livrează o sesiune Xorg (vedeți nota despre GNOME de mai jos) |
| Kali Linux (Xfce, X11) | ✅ | Rulează în sesiunea implicită Kali, Xfce pe X11, prin aceeași randare integrată ca pe celelalte desktopuri X11 |
| Xfce (X11) | ✅ | Pictogramele de pe desktop rămân vizibile peste imaginea de fundal și le puteți apăsa, selecta și trage |
| MATE (X11) | ✅ | Pictogramele de pe desktop rămân vizibile și pot fi apăsate peste imaginea de fundal |
| Deepin 25 (DDE, X11) | ✅ | Adaptare automată la DDE — verificat de comunitate pe Deepin 25 Community build1 |
| COSMIC (Wayland) | ✅ | layer-shell |
| Hyprland | ✅ | layer-shell |
| Sway | ✅ | layer-shell |
| Cinnamon 6.8 și mai nou (Wayland) | ✅ | Compozitorul Cinnamon a primit layer-shell în 6.8. Cinnamon 6.6 și mai vechi nu au layer-shell și rămân cu un cadru fix pe Wayland |
| KDE Plasma 6 (X11 și Wayland) | ✅ | Setat prin pluginul de imagine de fundal al plasmashell, deci pictogramele de pe desktop rămân vizibile. Pentru video are nevoie de `qml6-module-qtmultimedia`. Redarea este cea a Qt, nu a mpv: fără sunet, fără reglaje hwdec, decupare sau tranziții; o listă de redare rulează primul fișier, iar o succesiune de imagini afișează primul cadru. Opțiunea Umplere (Acoperire, Încadrare, Întindere) este respectată |
| GNOME pe Wayland | ⚠️ | Doar cadru fix — Mutter nu expune nicio suprafață pentru fundal animat, deci nu există video și nici widgeturi. Videoul live are nevoie de o extensie Fresco pentru GNOME (planificată) |

Fiecare mediu de mai sus este testat fără interfață grafică (headless) în CI la fiecare versiune.

**Notă despre GNOME.** GNOME pe Wayland este singurul desktop popular pe care Fresco poate afișa doar un cadru fix. Vechea soluție, autentificarea într-o sesiune Xorg, dispare: GNOME 49 a dezactivat sesiunea X11, iar GNOME 50 a eliminat-o, așa că Ubuntu 25.10 și mai noi (inclusiv 26.04 LTS) și Fedora 43 și mai noi nu livrează nicio sesiune Xorg la care să treceți. Ubuntu 22.04 și 24.04 oferă încă „Ubuntu on Xorg”. Unde nu există sesiune Xorg, videoul live pe desktopul GNOME standard trebuie să aștepte o extensie Fresco pentru GNOME, care este planificată; Fresco spune asta în aplicație, în loc să se prefacă că videoul rulează. KDE Plasma, COSMIC, Hyprland, Sway și toate desktopurile X11 nu sunt afectate.

> „Ușor de folosit, cu o interfață curată — una dintre puținele aplicații de fundal animat adaptate corect pentru Deepin 25, instalabilă prin .deb și care rulează fluid, cu redare accelerată hardware.”
>
> — 柒玖 (forumul deepin) / 柒仈玖 (GitHub), testat pe Deepin 25 Community build1, sesiune X11, Intel Alder Lake-N [Intel Graphics]

Deepin 25 livrează X11 ca sesiune implicită, iar aceasta este sesiunea pe care Fresco este verificat acolo. Compozitorul Wayland propriu al Deepin, [Treeland](https://github.com/linuxdeepin/treeland), este încă în dezvoltare, așa că Fresco nu face deocamdată nicio afirmație despre Deepin pe Wayland.

## Fresco vs Wallpaper Engine, Hidamari, Komorebi și mpvpaper

| Instrument | Platformă | Wayland | X11 | Interfață | Redă | Licență |
|---|---|---|---|---|---|---|
| **Fresco** | Linux | ✅ compozitoare layer-shell și KDE Plasma 6; cadru fix pe GNOME | ✅ | Aplicație GTK4, plus un CLI `fresco` | Video, GIF, imagini, succesiuni de imagini, liste de redare | GPL-3.0-or-later |
| **Wallpaper Engine** | Windows (Steam); fără versiune oficială pentru Linux | n/a | n/a | Aplicație grafică | Fundaluri video, de tip scenă și web | Cu plată, sursă închisă |
| **[linux-wallpaperengine](https://github.com/Almamu/linux-wallpaperengine)** | Linux; are nevoie de resursele unei copii Steam a Wallpaper Engine | ✅ compozitoare cu layer-shell | ⚠️ nu atunci când un mediu desktop (GNOME, KDE) desenează fundalul | Linie de comandă; README-ul lui listează interfețe grafice separate, făcute de comunitate | Fundaluri Wallpaper Engine | GPL-3.0 |
| **[Hidamari](https://github.com/jeffshee/hidamari)** | Linux, orientat spre GNOME | ⚠️ GNOME pe Wayland | ✅ | Aplicație grafică | Video | GPL-3.0 |
| **[Komorebi](https://github.com/cheesecakeufo/komorebi)** | Linux | — | — | Aplicație grafică | Fundaluri de tip imagine, video și pagină web | GPL-3.0 |
| **[mpvpaper](https://github.com/GhostNaN/mpvpaper)** | Linux, compozitoare bazate pe wlroots | ✅ layer-shell | ❌ | Linie de comandă | Video, prin mpv | GPL-3.0 |
| **xwinwrap** | Linux, doar X11 | ❌ | ✅ | Linie de comandă | Orice player atașați (mpv, gifview) | — |
| **swww** (acum [awww](https://codeberg.org/LGFae/awww)) | Linux, compozitoare layer-shell; nu GNOME | ✅ | ❌ | Daemon în linie de comandă | GIF-uri animate și imagini, nu video | GPL-3.0 |

Fresco include `mpvpaper` ca randare pentru Wayland, deci se sprijină pe acel proiect, nu îi face concurență. Un `—` înseamnă că documentația proprie a proiectului nu spune clar. Verificat pe depozitul public al fiecărui proiect la 2026-10-16: ultimul push în depozitul Komorebi a fost în iulie 2025, iar depozitul swww de pe GitHub este arhivat, deoarece proiectul s-a mutat pe Codeberg sub numele awww.

## Performanță și autonomie

柒玖 (forumul deepin) / 柒仈玖 (GitHub) a măsurat puterea pachetului cu `turbostat` pe un Intel N150 (Deepin 25, VA-API, două rulări pe nivel) în timp ce rula o imagine de fundal video:

| Video | Economisire energie | Consum GPU | Consum total al pachetului |
|---|---|---|---|
| 1080p 60fps | Calitate maximă | 1,37 W | 6,00 W |
| 1080p 60fps | **Redusă** (implicit) | **0,63 W** (−54%) | **4,03 W** (−33%) |
| 4K 60fps | Calitate maximă | 2,77 W | 7,94 W |
| 4K 60fps | **Redusă** (implicit) | **1,60 W** (−42%) | **5,95 W** (−25%) |
| 4K 60fps | Minim | 0,99 W (−65%) | 4,97 W (−37%) |

Economisirea energiei reduce costul scalării GPU pentru fiecare cadru. Nu se pierd cadre, iar decodarea hardware rămâne neatinsă, deci redarea rămâne fluidă — compromisul este claritatea imaginii, nu mișcarea. Redusă este nivelul implicit; Minim merită ales pentru surse 4K.

**Cum se citesc aceste cifre.** Decodarea hardware nu face un fundal animat gratuit — mută munca de pe procesor pe motorul video, unde este mai ieftină, nu inexistentă. „Utilizare redusă a procesorului” spus singur e o afirmație slabă, pentru că alternativa cu care se compară este decodarea software, mult mai proastă; măsura sinceră este consumul întregului sistem, de aceea tabelul arată **consumul total al pachetului**, nu doar al GPU-ului.

Două rezerve despre cifrele de mai sus, ca să nu fie citite ca mai mult decât sunt:

- Sunt **consumul total al pachetului în timp ce rulează o imagine de fundal**, nu costul marginal al imaginii de fundal. Nu s-a înregistrat nicio valoare de repaus pe aceeași mașină, deci diferența dintre aceste cifre și un desktop inactiv nu este stabilită aici. Procentele compară *nivelurile* de economisire a energiei între ele, iar pentru asta sunt valabile.
- Sunt de pe o singură mașină (Intel N150, Alder Lake-N, VA-API, Deepin 25, două rulări pe nivel). GPU-urile dedicate, NVDEC și alte drivere vor diferi.

Un fundal animat costă întotdeauna mai mult decât unul static. Economisirea energiei, pauza automată la ecran complet și pauza pe baterie există ca să limiteze acest cost, nu ca să pretindă că este zero.

## Întrebări frecvente (FAQ)

### Există o alternativă la Wallpaper Engine pentru Linux?

Da. Fresco este o aplicație Linux gratuită și open source care redă propriile dvs. fundaluri video, GIF și imagine printr-o interfață grafică, cu decodare hardware și fără strat de compatibilitate. Nu deschide fișierele de fundal ale Wallpaper Engine; dacă dețineți Wallpaper Engine și doriți acele fundaluri pe Linux, proiectul separat [linux-wallpaperengine](https://github.com/Almamu/linux-wallpaperengine) le rulează din linia de comandă și are nevoie de resursele unei copii Steam a Wallpaper Engine. Vedeți [tabelul comparativ](#fresco-vs-wallpaper-engine-hidamari-komorebi-și-mpvpaper).

### Funcționează Wallpaper Engine pe Linux?

Nu nativ. Wallpaper Engine este o aplicație Windows, fără versiune oficială pentru Linux. Fresco este o alternativă nativă pentru Linux, care se instalează ca `.deb` și nu are nevoie de niciun strat de compatibilitate.

### Cum setez un videoclip ca imagine de fundal în Ubuntu?

Instalați Fresco, deschideți-l, faceți clic pe **Adăugare**, alegeți videoclipul și faceți clic pe **Setare ca imagine de fundal**. Videoclipul rulează ca fundal de desktop și este restaurat la autentificare. Sesiunea implicită Ubuntu, GNOME pe Wayland, poate afișa doar un cadru fix. Pe Ubuntu 22.04 și 24.04 vă puteți autentifica într-o sesiune **Ubuntu on Xorg** pentru redare animată completă; Ubuntu 25.10 și mai noi (inclusiv 26.04 LTS) nu livrează sesiune Xorg, așa că acolo videoul live pe desktopul GNOME standard are nevoie de o extensie Fresco pentru GNOME, care este planificată. Variantele Ubuntu care folosesc KDE Plasma, Xfce sau MATE nu sunt afectate.

### Cum obțin un fundal animat în Ubuntu, Linux Mint, Pop!_OS, Fedora sau Arch Linux?

Instalați `.deb`-ul Fresco pe Ubuntu, Linux Mint și Pop!_OS sau compilați-l din sursă pe Fedora și Arch Linux. Comenzile pentru fiecare se găsesc în [Instalare](#instalare).

- **Ubuntu:** `.deb`. Pe desktopul GNOME implicit, redarea live cere o sesiune Xorg (Ubuntu 22.04 și 24.04); variantele cu KDE Plasma, Xfce și MATE nu sunt afectate — vedeți răspunsul despre Ubuntu de mai sus.
- **Linux Mint:** `.deb`. Cinnamon pe X11 este compatibil, iar Cinnamon pe Wayland începând cu versiunea 6.8.
- **Pop!_OS:** `.deb`. Desktopul COSMIC (Wayland layer-shell) este compatibil.
- **Fedora:** compilare din sursă. GNOME din Fedora 43 și mai noi este doar Wayland, deci afișează un cadru fix; KDE Plasma și desktopurile X11 sunt compatibile.
- **Arch Linux:** compilare din sursă și instalarea `mpvpaper` din AUR (`yay -S mpvpaper`).

Doar calea `.deb` este acoperită de CI-ul Fresco.

### Ce distribuții Linux sunt compatibile?

Fresco livrează un pachet `.deb` pentru distribuțiile bazate pe Debian și Ubuntu: Ubuntu, Pop!_OS, Linux Mint, Debian, elementary OS, Deepin 25 și Kali Linux. Alte distribuții pot compila din sursă — vedeți [Instalare](#instalare) și [docs/INSTALL.md](docs/INSTALL.md).

### Folosesc fundalurile animate mult procesor, GPU sau baterie?

Procesor, nu. Baterie, ceva — un fundal animat nu este niciodată gratuit.

Fresco decodează videoul pe GPU (VA-API / NVDEC), deci utilizarea procesorului rămâne aproape de zero. Asta mută costul pe motorul video, în loc să-l elimine. Măsurat pe un Intel N150, la nivelul implicit de Economisire energie, o imagine de fundal 1080p a consumat 0,63 W pe GPU, cu un consum total al pachetului de 4,03 W în timpul redării (vedeți [Performanță](#performanță-și-autonomie) pentru cum se citește asta, inclusiv ce *nu* spune). Pauza automată la ecran complet și pauza pe baterie există ca să țină acest cost departe de baterie atunci când ar conta cel mai mult.

### Funcționează Fresco pe Wayland?

Da, pe compozitoarele care implementează protocolul layer-shell — COSMIC, Hyprland și Sway — și pe KDE Plasma 6 (X11 și Wayland) prin pluginul de imagine de fundal al plasmashell. GNOME pe Wayland este excepția: Mutter nu expune nicio suprafață pentru fundal, deci Fresco afișează acolo un cadru fix și o spune în aplicație. Sesiunile X11 sunt compatibile în totalitate.

### Funcționează Fresco pe GNOME?

Da, pe X11, cu fundalul animat complet — video, widgeturi, totul. Pe **GNOME sub Wayland**, Mutter nu expune nicio suprafață de fundal în care o aplicație să deseneze, deci Fresco afișează acolo un cadru fix, iar widgeturile nu sunt disponibile. Dacă distribuția dvs. mai livrează o sesiune Xorg (Ubuntu 22.04 și 24.04 o fac), deconectați-vă și alegeți „GNOME on Xorg” sau „Ubuntu on Xorg” pentru redare animată completă. GNOME 49 a dezactivat acea sesiune, iar GNOME 50 a eliminat-o, deci Ubuntu 25.10 și mai noi (inclusiv 26.04 LTS) și Fedora 43 și mai noi nu o au: pe acestea, videoul live pe GNOME are nevoie de o extensie Fresco pentru GNOME, care este planificată. `fresco doctor` vă spune în ce situație vă aflați.

### Funcționează Fresco pe KDE Plasma, COSMIC, Hyprland sau Sway?

Da, pe toate patru. COSMIC, Hyprland și Sway redau video live prin layer-shell. KDE Plasma 6 (X11 și Wayland) folosește propriul plugin de imagine de fundal al Fresco în plasmashell, care păstrează pictogramele de pe desktop vizibile, dar redă prin Qt în loc de mpv: fără sunet, fără decupare sau tranziții, și are nevoie de `qml6-module-qtmultimedia` pentru video. Opțiunea Umplere (Acoperire, Încadrare sau Întindere) se aplică și acolo.

### Funcționează Fresco pe Linux Mint, Xfce, Cinnamon sau MATE?

Da. Cinnamon, Xfce și MATE pe X11 folosesc randarea integrată, iar pictogramele de pe desktop rămân vizibile peste imaginea de fundal pe Xfce și MATE; pe Xfce le puteți apăsa, selecta și trage ca de obicei. Cinnamon pe Wayland funcționează începând cu Cinnamon 6.8, care a adăugat layer-shell.

### Cum schimb imaginea de fundal cu o scurtătură de tastatură?

Asociați `fresco next`, `fresco prev` sau `fresco random` unei taste în setările compozitorului sau ale desktopului — ele schimbă imaginea de fundal fără să deschidă aplicația (`random` nu o alege niciodată pe cea care rulează deja). Parcurg biblioteca în ordinea în care o arată aplicația, sau doar dosarul pe care îl aveți deschis acolo, și pornesc serviciul Fresco dacă nu rulează. Dacă fereastra Fresco este deschisă în acel moment, ea preia noua imagine de fundal în loc să o repună pe cea veche. În aplicație, Ctrl+K deschide o paletă de comenzi. De exemplu, în Sway:

```
bindsym $mod+Right  exec fresco next
bindsym $mod+Left   exec fresco prev
bindsym $mod+Return exec fresco random
```

### Pot folosi o imagine de fundal diferită pe fiecare monitor?

Da. Fresco acceptă imagini de fundal separate pentru fiecare ecran, iar când același videoclip este folosit pe mai multe monitoare, redarea este ținută în sincron.

### Acceptă GIF-uri și succesiuni de imagini?

Da — GIF-uri animate, imagini statice, succesiuni de imagini cu tranziții (fondare prin negru, Ken Burns, apropiere și neclaritate) și liste de redare cu mai multe videoclipuri, pe lângă fișierele video.

### Poate Fresco să folosească dosarul meu existent cu imagini de fundal fără import?

Da. Alegeți **Asociere dosar…** din meniu (sau **Asociază acest dosar** după **Adăugare dosar**) pentru a indica Fresco un dosar în care păstrați deja imagini de fundal. Imaginile, GIF-urile și videoclipurile din el apar în bibliotecă, într-o secțiune proprie, acolo unde se află, fără să fie importate, iar biblioteca rămâne sincronizată pe măsură ce adăugați și eliminați fișiere în acel dosar. **Include subdosarele** este opțional (până la patru niveluri de adâncime), iar favoritele și modificările sunt reținute pentru fiecare fișier. Dacă dosarul dispare, de exemplu pentru că unitatea a fost deconectată, secțiunea lui rămâne marcată „(indisponibil)” și revine singură. **Anulare asociere dosar**, din meniul secțiunii, elimină secțiunea fără să șteargă niciun fișier. **Adăugare** importă în continuare fișiere individuale, ca înainte.

### Poate Fresco să gestioneze mii de imagini de fundal?

Da. Biblioteca construiește doar cardurile imaginilor de fundal din apropierea locului în care derulați, nu pe toate deodată, așa că o colecție mare nu îngreunează aplicația. Pe o bibliotecă de test cu 4.000 de imagini de fundal (jumătate imagini, jumătate videoclipuri), biblioteca a apărut în aproximativ 2 secunde în loc de 9 până la 13, iar aplicația a folosit aproximativ 325 MB de memorie în loc de 1,6 GB. Aceste cifre provin dintr-o versiune de dezvoltare, sub sarcină, pe o singură mașină, așa că tratați-le ca pe o orientare, nu ca pe o promisiune.

### Pot folosi un videoclip YouTube ca imagine de fundal?

Nu lipindu-i linkul. **Din link** acceptă pinuri Pinterest și linkuri directe către un fișier video sau imagine (mp4, webm, mkv, mov, gif, jpg, png, webp); alte linkuri sunt respinse cu un mesaj. Adăugați cu **Adăugare** un fișier video pe care îl aveți deja sau lipiți un link direct către fișier.

### Este Fresco gratuit și open source?

Da. Fresco este gratuit și open source sub GPL-3.0-or-later. Nu există reclame, conturi sau nivel cu plată.

### Colectează Fresco date despre mine?

Nu se trimite nimic până nu răspundeți la o casetă de dialog de consimțământ, afișată o singură dată. Alegeți între o numărătoare zilnică anonimă (ID de instalare aleatoriu, țară, versiunea aplicației) și aceasta plus detalii opționale; dacă setați `telemetry = false` în `~/.config/fresco/config.toml`, nu se trimite absolut nimic. Vedeți [Confidențialitate și termeni de utilizare](#confidențialitate-și-termeni-de-utilizare) pentru fiecare câmp.

### Care este diferența dintre Fresco și mpvpaper?

`mpvpaper` este un înveliș în linie de comandă pentru mpv, care redă un singur videoclip ca fundal layer-shell pe Wayland — fără interfață grafică, fără bibliotecă, fără suport X11. Fresco este o aplicație desktop GTK4 construită în jurul lui: bibliotecă de imagini de fundal, editor de decupare și rotire, programare, sincronizare pe mai multe monitoare, widgeturi pentru desktop și suport X11, fără să fie nevoie de terminal. Fresco include `mpvpaper` ca randare pentru Wayland, în loc să-l înlocuiască — vedeți [tabelul comparativ](#fresco-vs-wallpaper-engine-hidamari-komorebi-și-mpvpaper).

### Pot folosi Fresco pe Kali Linux?

Da. Kali Linux se bazează pe Debian, deci Fresco se instalează la fel ca pe Debian sau Ubuntu — cu pachetul `.deb` sau cu scriptul de instalare dintr-o linie (vedeți [Instalare](#instalare)). Desktopul implicit Kali, Xfce pe X11, este compatibil prin randarea X11 integrată a Fresco.

### Cum elimin mai multe imagini de fundal deodată?

Faceți clic pe **Selectare** în subsol (sau clic dreapta pe o imagine de fundal și alegeți **Selectare…**), bifați-le pe cele dorite și faceți clic pe **Eliminare**. **Selectare totală** respectă căutarea curentă, deci puteți căuta mai întâi și apoi goli un întreg lot. Eliminarea unei imagini de fundal o scoate din biblioteca Fresco — fișierul sursă de pe disc rămâne.

### Cum dezinstalez Fresco?

Dacă ați instalat `.deb`-ul sau ați folosit scriptul de instalare, rulați `sudo apt remove fresco`. Biblioteca și configurația se află în `~/.local/share/fresco/` și `~/.config/fresco/`; ștergeți aceste directoare ca să eliminați orice urmă. Ca să recăpătați imaginea de fundal proprie a desktopului fără dezinstalare, deschideți Fresco și faceți clic pe **Oprire imagine de fundal**.

## Widgeturi pentru desktop: versuri, ceasuri, vizualizator și copertă de album

Fresco desenează cinci widgeturi opționale peste imaginea de fundal. Toate sunt **dezactivate implicit**; activați-le din meniul aplicației (Ctrl+,) → **Avansat…**.

| Widget | Ce primiți |
|---|---|
| **Versuri** | Rândul curent, în ritmul muzicii. Patru presetări (Minimal, Karaoke, Subtitrare, Card), o grilă de poziționare cu nouă puncte, un glisor pentru decalajul de sincronizare, un rând următor estompat opțional și titlul și artistul melodiei, opționale |
| **Ceas** | Opt teme — Digital, Minimal, Segmentat, Stivuit, Text, Card, NOS și o față de ecran de blocare în stil telefon. Format de 12 sau 24 de ore, dată opțională, controale de culoare și opacitate. Secundele sunt dezactivate implicit, pentru că costă de 60× mai multe redesenări |
| **Ceas mondial** | Ora în mai multe orașe deodată, alese din lista completă de fusuri orare, ca text simplu pe 12 ore (de exemplu `Tokyo  9:41 PM`), cu o etichetă opțională pentru fiecare oraș. Se redesenează o dată pe minut. Nou în 1.1.50 |
| **Vizualizator audio** | Cinci stiluri — Bare, Oglindire, Undă, Puncte, Inel — cu selector de culoare, degrade din două culori sau curcubeu |
| **Copertă album** | Coperta melodiei curente pe un disc rotativ; se oprește din rotit când redarea este în pauză |

Cu excepția modului opțional **Plutește deasupra ferestrelor** al ceasului mondial, widgeturile sunt pictate în imaginea de fundal prin stratul OSD al mpv, nu într-o fereastră proprie, deci nu stau niciodată deasupra ferestrelor, nu interceptează niciodată un clic și se comportă identic pe X11 și pe orice compozitor layer-shell. Cu muzica pornită și cu widgeturile de versuri, ceas, vizualizator și copertă de album active, costul măsurat a fost de **0,8% dintr-un nucleu de procesor** — aproape în întregime din captura audio, deoarece nimic nu se redesenează dacă conținutul nu s-a schimbat. Widgeturile revin și singure după ce calculatorul trece în repaus și își reia activitatea.

Widgeturile apar implicit pe **toate ecranele**. Ca să le păstrați pe unul singur, adăugați `monitor = "DP-1"` în blocul `[widgets]` din `config.toml` (încă nu există un control grafic pentru asta). Ele **nu sunt disponibile pe GNOME sub Wayland**, care nu are o suprafață de fundal animat în care Fresco să deseneze — același motiv pentru care imaginile de fundal revin acolo la un cadru fix.

### Pot adăuga un ceas pe fundalul desktopului meu Linux?

Da. Activați widgetul **Ceas** pentru un ceas tematic desenat în imaginea de fundal sau **Ceas mondial** pentru a afișa mai multe orașe deodată. Ceasul mondial stă pe imaginea de fundal ca celelalte widgeturi; modul său opțional **Plutește deasupra ferestrelor** îl ridică deasupra tuturor aplicațiilor, cu clicurile trecând prin el, ceea ce funcționează pe X11 cu un compozitor și pe compozitoarele layer-shell (KDE Plasma, COSMIC, Sway, Hyprland) și revine la imaginea de fundal acolo unde sesiunea nu-l poate face să plutească, cum este GNOME pe Wayland.

### Pot afișa versuri pe desktopul meu Linux?

Da. Fresco desenează **versuri sincronizate în timp** peste imaginea de fundal, urmărind orice rulează în sistem prin MPRIS — browsere, aplicații de muzică, playere video. Alegeți o presetare, poziționarea și decalajul de sincronizare în setările widgetului Versuri (vedeți tabelul de mai sus).

### Are Linux widgeturi pentru desktop, ca Conky?

Da, iar Fresco adaugă cinci care nu au nevoie de panou, de extensie și nici de sprijin din partea desktopului: un ceas, un ceas mondial, versuri sincronizate, un vizualizator audio și un disc rotativ cu coperta albumului. Fiind pictate în imaginea de fundal, nu într-o fereastră, funcționează pe desktopuri care nu au un strat propriu de widgeturi — inclusiv COSMIC, Hyprland și Sway. Spre deosebire de Conky, Fresco **nu are widgeturi de monitorizare a sistemului** (fără indicatori de procesor, RAM, temperatură sau rețea), deci stă alături de Conky ca însoțitor de muzică și oră, nu îl înlocuiește. GNOME pe Wayland este singurul loc unde nu poate rula.

### Pot avea un vizualizator de muzică pe fundalul desktopului?

Da. Vizualizatorul audio al Fresco reacționează la orice redă sistemul, într-unul din cinci stiluri — Bare, Oglindire, Undă, Puncte sau Inel — cu selector de culoare, degrade din două culori sau curcubeu. Este dezactivat implicit și cere consimțământul prima dată când îl activați, pentru că trebuie să asculte ieșirea audio; consimțământul este verificat și la încărcarea configurației, deci editarea manuală a `config.toml` nu îl poate porni pe la spatele dvs. Asociați-l cu widgetul de copertă de album pentru un disc rotativ cu coperta melodiei curente.

### Ce playere de muzică funcționează cu widgetul de versuri?

Orice publică metadate MPRIS standard, dar nu sunt la fel de fiabile:

| Player | Funcționează? | Observații |
|---|---|---|
| **Firefox** | ✅ | Alegerea fiabilă și cea față de care a fost verificată funcția |
| Chrome / Brave / Edge / Vivaldi / Opera | ⚠️ | Vedeți mai jos |
| Spotify — **în browser** | ✅ | Raportează corect poziția de redare |
| Spotify — **clientul nativ pentru Linux** | ⚠️ | Raportează poziția 0 la nesfârșit, deci versurile nu pot rămâne sincronizate |
| Playere locale (VLC, mpv, Rhythmbox, …) | ✅ | Este și cazul în care un fișier `.lrc` local are cele mai multe șanse să existe |

Browserele din familia Chromium revendică un nume MPRIS prima dată când orice filă
redă conținut media și **nu-l mai eliberează niciodată** — un bug vechi din Chromium.
Când redarea se termină, titlul se golește, dar rămâne pe magistrală o sesiune
„zombi” învechită, uneori purtând încă o copertă. Fresco ignoră orice player care nu
publică un titlu de melodie, ceea ce sare peste acele sesiuni, dar raportarea
proprie a Chromium rămâne suficient de inconsecventă încât Firefox este browserul
de folosit pentru asta.

Clientul nativ Spotify pentru Linux returnează `Position: 0` și nu a emis niciodată
un seek din 2018, atât în variantele native, cât și în cele snap. Fresco detectează
asta comportamental — trei citiri zero, la intervale, în timpul redării — și rulează
liber ceasul versurilor de la schimbarea melodiei, ceea ce se decalează dacă săriți
prin melodie. Spotify în browser nu are această problemă.

### De unde provin versurile?

Mai întâi dintr-un fișier `.lrc` local: unul aflat lângă fișierul audio sau unul
corespunzător dintr-un dosar de versuri pe care îl indicați lui Fresco. Această
cale funcționează offline și este cea mai bună potrivire, pentru că este fișierul
pe care l-ați ales.

Când nu există fișier local — ceea ce se întâmplă de cele mai multe ori dacă
ascultați în streaming —, Fresco caută melodia pe [LRCLIB](https://lrclib.net), o
bază de date gratuită de versuri sincronizate, întreținută de comunitate, și
păstrează răspunsul în cache în `~/.cache/fresco/lyrics`, astfel încât aceeași
melodie să nu fie descărcată de două ori. Căutarea trimite către LRCLIB titlul,
artistul și albumul melodiei; nu se trimite nimic decât dacă widgetul de versuri
este activ și melodia nu are fișier local. Fresco nu găzduiește, nu deține și nu
licențiază conținut de versuri — intrările LRCLIB sunt contribuite de utilizatorii
săi, iar LRCLIB nu declară nicio licență asupra lor, așa că Fresco le preia la
cerere și le păstrează în cache pentru fiecare utilizator, în loc să livreze o bază
de date proprie de versuri.

Sincronizarea versurilor din fișierele `.lrc` este făcută de mână și adesea puțin
decalată. Există un glisor pentru decalajul de sincronizare în setările de versuri
tocmai pentru asta.

## Depanare

Rulați întâi `fresco doctor`: raportează sesiunea dvs., ce randare a ales Fresco și de unde provine, și dacă se aplică soluția ocolitoare Xorg pentru GNOME. [docs/INSTALL.md](docs/INSTALL.md) acoperă mai multe cazuri, inclusiv utilizarea ridicată a procesorului. Includeți ieșirea comenzii `fresco doctor` când deschideți un issue.

Tot nu merge? [Raportați un bug](https://github.com/DibbayajyotiRoy/Fresco/issues/new?template=bug_report.yml) sau [cereți o funcție](https://github.com/DibbayajyotiRoy/Fresco/issues/new?template=feature_request.yml).

### Fresco nu apare în lansatorul Deepin după instalare — de ce?

O problemă cunoscută a dde-launchpad: reîmprospătarea de după instalare nu preia
Fresco. Rulați `killall dde-shell` (repornește singur) sau deconectați-vă și
reconectați-vă, iar intrarea apare definitiv. Fresco în sine se instalează corect —
este listat de managerul de aplicații al Deepin, iar pictograma lui se rezolvă în
fiecare temă instalată. Dacă se întâmplă la fiecare reinstalare pe o mașină care a
avut Fresco și înainte, rulați `sh scripts/dde-launcher-diag.sh` — este doar pentru
citire și afișează un jurnal care merită atașat la un issue.

### Pictogramele de pe desktop sunt ascunse cât timp rulează fundalul pe Deepin

Deepin 25 desenează imaginea de fundal și pictogramele desktopului într-o singură
fereastră opacă, deci un fundal animat pe care îl puteți vedea este unul așezat
deasupra acelei ferestre — nu există un strat care să se afle între ele. **Faceți
clic pe desktop și pictogramele revin pentru zece secunde**, suficient cât să
deschideți ce căutați; după aceea fundalul revine, iar un alt clic cumpără alte
zece secunde. Ca să schimbați durata, setați `dde_icon_peek_secs` în
`~/.config/fresco/config.toml` (`0` ține fundalul mereu deasupra). Imaginea de
fundal proprie a Deepin se vede în continuare ori de câte ori Fresco este oprit sau
în pauză.

### De ce arată întunecat fundalul desktopului MATE după un crash?

Pentru a ține pictogramele desktopului Caja deasupra fundalului animat, Fresco
setează fundalul desktopului MATE la o culoare-cheie aproape neagră (`#010101`) cât
timp rulează și copiază pe imaginea de fundal tot ce nu are acea culoare —
pictogramele. Fundalul dvs. este salvat mai întâi și pus la loc când Fresco se oprește
sau este dezactivat. Dacă Fresco cade înainte să apuce să facă asta, desktopul
rămâne întunecat; **porniți din nou Fresco și vă restaurează fundalul**. Pe un
server X fără extensiile Composite și Damage, Fresco funcționează în schimb ca pe
[Deepin](#pictogramele-de-pe-desktop-sunt-ascunse-cât-timp-rulează-fundalul-pe-deepin):
pictograme ascunse, iar un clic pe desktop le readuce.

### Imaginea mea de fundal este neagră pe Wayland (NVIDIA, COSMIC, Hyprland, Sway)

O imagine de fundal neagră — cu totul în rest funcționând și fără erori în jurnal —
înseamnă aproape întotdeauna că randarea mpvpaper inclusă de Fresco este prea
veche pentru configurația dvs. Versiunile dinainte de 1.6 inițializează EGL,
raportează succes și apoi nu prezintă niciodată un cadru pe driverul proprietar
NVIDIA.

Rulați întâi `fresco doctor`. Afișează randarea aleasă, de unde provine acel binar
(inclus în pachet sau instalat de dvs.) și aproximativ ce versiune este, și
avertizează când versiunea este mai veche decât remedierea:

```
  ✓ mpvpaper available (/usr/lib/fresco/mpvpaper-libmpv2)
      source: bundled · version: 1.4–1.6
  ⚠ mpvpaper may be too old …
```

Fresco preferă deja un `mpvpaper` mai nou din `PATH` în locul unuia inclus mai
vechi, deci instalarea pachetului `mpvpaper` al distribuției (sau compilarea
[upstream](https://github.com/GhostNaN/mpvpaper) cu `scripts/build-mpvpaper.sh`)
este de obicei suficientă. Ca să îndreptați Fresco spre un binar anume, setați
**`FRESCO_MPVPAPER`** la calea lui completă — aceasta are prioritate față de toate
celelalte alegeri:

```bash
mkdir -p ~/.config/environment.d
echo 'FRESCO_MPVPAPER=/home/YOU/.local/bin/mpvpaper' > ~/.config/environment.d/fresco.conf
```

Deconectați-vă și reconectați-vă (systemd citește `environment.d` la începutul
sesiunii), apoi confirmați cu `fresco doctor` că linia `source:` spune acum
`FRESCO_MPVPAPER override`.

În mod similar, **`FRESCO_HWDEC`** suprascrie decodorul hardware al mpv (de
exemplu `nvdec`, `nvdec-copy`, `vaapi`, `no`) — un instrument de diagnosticare
pentru utilizarea ridicată a procesorului. Implicit, Fresco alege
`nvdec,vaapi,auto-safe` când există un GPU NVIDIA, altfel `auto-safe` (videoul
rotit folosește copy-back: `nvdec-copy,auto-copy` / `auto-copy`).

## Confidențialitate și termeni de utilizare

**Nu se trimite nimic până nu răspundeți.** O casetă de dialog de consimțământ întreabă o dată, la prima pornire, iar alegerea poate fi schimbată oricând în Setări.

Oricum ați alege, o dată pe zi Fresco înregistrează că o instalare a fost activă, în ce țară, pe ce versiune. Aceasta este numărătoarea. Ce întreabă caseta de dialog sunt **detaliile**:

| Alegerea dvs. | Ce trimite Fresco |
| --- | --- |
| **Acceptați tot** | Numărătoarea, plus distribuția, desktopul, tipul de sesiune, backendul video, numărul de monitoare, ce funcții folosiți, tipurile de erori, orașul și regiunea și ora exactă a fiecărei verificări. |
| **Refuzați opționalul** | Doar numărătoarea: un ID de instalare aleatoriu (niciodată derivat din hardware-ul sau numele dvs.), țara, versiunea aplicației, împachetarea. Verificarea dvs. este stocată ca **dată, nu ca oră**. |

**Orașul și ora exactă a utilizării sunt opționale** — trimise doar dacă acceptați tot, niciodată trimise dacă refuzați. Coordonatele nu sunt colectate niciodată, la niciun nivel: endpointul de geolocalizare renunță la latitudine și longitudine în loc să le returneze.

Aceste cifre există ca să decidă ce distribuții și desktopuri sunt testate înainte de o versiune și unde descărcările au nevoie de o oglindă. Nu sunt niciodată vândute, partajate sau folosite pentru publicitate și nu este implicat niciun furnizor de analiză.

**Niciodată colectate, în niciun caz:** date personale, nume de fișiere, imaginile dvs. de fundal, adresa IP, sunet, apăsări de taste sau conținutul clipboardului. Țara este determinată din IP-ul dvs. de către Cloudflare la marginea rețelei, deci proiectul primește doar un cod de două litere. (Textul pe care îl scrieți deliberat și îl trimiteți — un comentariu de feedback sau un mesaj către întreținător, mai jos — este un mesaj pe care ați ales să-l trimiteți, nu colectare.)

Pentru a nu trimite nimic, setați `telemetry = false` și `telemetry_prompted = false` în `~/.config/fresco/config.toml`.

### Discutați anonim cu întreținătorul

**Meniu → Trimiteți un mesaj întreținătorului** deschide o conversație privată, în ambele sensuri, cu persoana care face Fresco. Fără cont, fără adresă de e-mail, fără autentificare GitHub. Este anonimă în ambele direcții: nu aflați cine este el dincolo de „întreținătorul”, iar el nu află cine sunteți — doar mesajele dvs. și, dacă lăsați caseta bifată, rezumatul configurației afișat în dialog.

Conversația este identificată printr-un tichet aleatoriu generat separat de telemetrie și stocat într-un alt fișier, deci o conversație nu poate fi niciodată asociată cu un profil de utilizare. Funcționează exact la fel, indiferent dacă ați acceptat tot sau ați refuzat opționalul. Nimic nu există până nu trimiteți un prim mesaj.

📄 **[Termenii de utilizare și politica de confidențialitate complete →](TERMS.md)** — fiecare câmp, într-un tabel, fără nimic omis. Fiecare linie care trimite ceva se află în [`src/telemetry.rs`](src/telemetry.rs), așa că puteți verifica în loc să aveți încredere.

## Detalii despre proiect

| | |
|---|---|
| **Nume** | Fresco (ID aplicație `io.github.dibbayajyotiroy.Fresco`) |
| **Ce este** | O aplicație de fundal animat și imagine de fundal video pentru desktopul Linux |
| **Licență** | [GPL-3.0-or-later](LICENSE) — gratuit și open source, fără reclame și fără cont |
| **Limbaj și stack** | Rust, GTK4 / libadwaita, libmpv; o randare mpvpaper inclusă pe Wayland |
| **Prima versiune** | v0.0.1, iunie 2026 |
| **Ultima versiune** | 1.1.53 (2026-10-28) — vedeți [Releases](https://github.com/DibbayajyotiRoy/fresco/releases/latest) pentru cea mai nouă |
| **Pachete** | `.deb` (amd64), script de instalare dintr-o linie, magazinul de aplicații Deepin; compilare din sursă în rest |
| **Limbile interfeței** | Engleză plus 13 traduceri: chineză simplificată, rusă, spaniolă, franceză, germană, portugheză (Brazilia), turcă, vietnameză, indoneziană, hindi, bengaleză, română și arabă |
| **Utilizatori** | Peste 3.000 de persoane din 128 de țări |
| **Întreținător** | Dibbayajyoti Roy |
| **Cod sursă** | <https://github.com/DibbayajyotiRoy/fresco> |
| **Site web** | <https://fresco.dibbayajyoti.com> |

## Contribuții și feedback

Rapoartele de bug-uri, cererile de funcții și PR-urile sunt binevenite — deschideți un [issue](https://github.com/DibbayajyotiRoy/fresco/issues) sau folosiți dialogul de feedback din aplicație. Vedeți [CONTRIBUTING.md](CONTRIBUTING.md).

## Licență

[GPL-3.0-or-later](LICENSE) — gratuit și open source.

---

<sub>Fresco este o aplicație gratuită și open source de fundal animat și imagine de fundal video pentru Linux (X11 și Wayland), cu widgeturi de versuri, ceas, vizualizator audio și copertă de album desenate în imaginea de fundal. Este o alternativă nativă la Wallpaper Engine pentru Linux și un însoțitor de muzică și ceas pentru instrumente precum Conky. Folosit de peste 3.000 de persoane din 128 de țări. Ultima actualizare: 2026-10-28.</sub>
