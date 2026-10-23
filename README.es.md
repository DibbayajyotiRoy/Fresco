<div align="center">

<img src="./data/icons/hicolor/256x256/apps/io.github.dibbayajyotiroy.Fresco.png" width="112" alt="Icono de la aplicación Fresco: una aplicación gratuita y de código abierto de fondos de pantalla en vivo para Linux" />

# Fresco — Aplicación gratuita de fondos de pantalla en vivo, animados y de video para Linux

[English](README.md) | [বাংলা](README.bn.md) | [Deutsch](README.de.md) | **Español** | [Français](README.fr.md) | [हिन्दी](README.hi.md) | [Bahasa Indonesia](README.id.md) | [Português (Brasil)](README.pt-BR.md) | [Română](README.ro.md) | [Русский](README.ru.md) | [Türkçe](README.tr.md) | [Tiếng Việt](README.vi.md) | [简体中文](README.zh-CN.md)

**Fresco es una aplicación gratuita y de código abierto de fondos de pantalla en vivo para Linux: convierte cualquier video, GIF o imagen en un fondo de pantalla animado del escritorio, tanto en X11 como en Wayland.** Es una alternativa nativa a Wallpaper Engine para Linux, con una aplicación gráfica, reproducción acelerada por GPU y widgets de escritorio.

[![Última versión de Fresco en GitHub](https://img.shields.io/github/v/release/DibbayajyotiRoy/fresco?style=flat-square&label=release)](https://github.com/DibbayajyotiRoy/fresco/releases/latest)
[![Licencia GPL-3.0-or-later](https://img.shields.io/github/license/DibbayajyotiRoy/fresco?style=flat-square)](LICENSE)
[![Estado del flujo de publicación](https://img.shields.io/github/actions/workflow/status/DibbayajyotiRoy/fresco/publish.yml?style=flat-square&label=publish)](https://github.com/DibbayajyotiRoy/fresco/actions/workflows/publish.yml)
[![Estrellas en GitHub](https://img.shields.io/github/stars/DibbayajyotiRoy/fresco?style=flat-square)](https://github.com/DibbayajyotiRoy/fresco/stargazers)

[Sitio web](https://fresco.dibbayajyoti.com) · [Instalación](#instalación) · [Entornos compatibles](#entornos-compatibles) · [Comparativa](#fresco-frente-a-wallpaper-engine-hidamari-komorebi-y-mpvpaper) · [Preguntas frecuentes](#preguntas-frecuentes-faq) · [Widgets](#widgets-de-escritorio-letras-relojes-visualizador-y-portada-del-álbum) · [Registro de cambios](CHANGELOG.md) · [Issues](https://github.com/DibbayajyotiRoy/fresco/issues)

<img src="./data/screenshots/gallery.png" alt="Biblioteca de fondos de pantalla de Fresco en un escritorio Linux oscuro: una cuadrícula con búsqueda de fondos de video, imagen y presentación, con un video marcado como ACTIVO y un indicador de estado que muestra la decodificación por hardware NVDEC" width="800" />

</div>

## ¿Qué es Fresco?

Fresco convierte un video, GIF, imagen, presentación de imágenes o lista de reproducción en el fondo de pantalla que se ve detrás de tus ventanas, mediante una aplicación GTK4 / libadwaita que no necesita terminal. La reproducción pasa por mpv: la decodificación se hace en la GPU y el uso de CPU se mantiene casi en reposo (en [Rendimiento y duración de la batería](#rendimiento-y-duración-de-la-batería) encontrarás lo que eso cuesta en el enchufe). Tu fondo de pantalla se restaura en cada inicio de sesión.

- **Reproduce:** videos (mp4, webm, mkv, avi, mov), GIF animados, imágenes, presentaciones de imágenes y listas de reproducción de videos.
- **Escritorios:** sesiones X11 (GNOME en Xorg, Cinnamon, Xfce, MATE, Deepin 25) y Wayland (COSMIC, Hyprland, Sway, KDE Plasma 6, Cinnamon 6.8+). GNOME en Wayland solo muestra una imagen fija; consulta [Entornos compatibles](#entornos-compatibles).
- **Decodificación por hardware:** VA-API (Intel, AMD) y NVDEC (NVIDIA), a través de mpv.
- **Biblioteca:** vincula una carpeta de fondos de pantalla que ya tengas en lugar de importarla, y mantenla sincronizada a medida que los archivos entran y salen. La biblioteca solo dibuja lo que está en pantalla, así que miles de fondos de pantalla siguen siendo ágiles y ligeros.
- **Widgets:** letras sincronizadas con la música, relojes (incluido un reloj mundial), un visualizador de audio y la portada del álbum, dibujados directamente en el fondo de pantalla. Todos desactivados por defecto.
- **Control:** la aplicación, una paleta de comandos con Ctrl+K y `fresco next` / `fresco prev` / `fresco random` para atajos de teclado.
- **Instalación:** un `.deb` para Ubuntu, Debian, Linux Mint, Pop!_OS, elementary OS, Kali y Deepin (script de una línea, descarga manual o la tienda de aplicaciones de Deepin); compilación desde el código fuente en Fedora, Arch y otras; consulta [Instalación](#instalación).
- **Licencia:** libre y de código abierto bajo [GPL-3.0-or-later](LICENSE), sin anuncios y sin cuenta.
- **Usuarios:** más de 3000 personas en 128 países. Última versión: 1.1.50.

## Instalación

Fresco se distribuye como paquete `.deb` para las distribuciones de la familia Debian y se compila desde el código fuente en cualquier otra.

### Ubuntu, Debian, Linux Mint, Pop!_OS, elementary OS y Kali Linux

Instalación con un solo comando: descarga el último `.deb` desde GitHub Releases y lo instala con apt:

```bash
curl -fsSL https://github.com/DibbayajyotiRoy/fresco/releases/latest/download/install.sh | FRESCO_SOURCE=github bash
```

O descarga el `.deb` desde [Releases](https://github.com/DibbayajyotiRoy/fresco/releases/latest) y ejecuta:

```bash
sudo apt install ./fresco_*.deb
```

### Deepin 25

Abre **App Store**, busca **Fresco** y haz clic en **Instalar**. Fresco está publicado en la tienda de aplicaciones de la comunidad de deepin, así que no hay nada que descargar a mano y las actualizaciones llegan por la tienda. La instalación de una línea de arriba también funciona en Deepin.

### Fedora, Arch Linux y otras distribuciones

Todavía no hay un paquete precompilado para estas distribuciones, así que hay que compilar desde el código fuente. Necesitas Rust (con [rustup](https://rustup.rs)) y los archivos de desarrollo de GTK4, libadwaita y libmpv:

```bash
git clone https://github.com/DibbayajyotiRoy/fresco.git && cd fresco
cargo build --release --all-features
```

En compositores Wayland, Fresco también necesita el renderizador `mpvpaper`, que el `.deb` incluye y una compilación desde el código fuente no:

```bash
# Arch Linux, CachyOS, EndeavourOS (AUR)
yay -S mpvpaper

# Fedora y cualquier otra: compílalo contra la libmpv de tu sistema
scripts/build-mpvpaper.sh
install -Dm755 target/release/mpvpaper ~/.local/bin/mpvpaper   # debe estar en el PATH
```

Fresco no está en el AUR ni en Flathub: el paquete del AUR llamado `fresco` es un programa de física sin relación. Los detalles y la solución de problemas están en [docs/INSTALL.md](docs/INSTALL.md).

Después de instalar, abre **Fresco** desde el lanzador de aplicaciones. Si algo no se ve bien, ejecuta `fresco doctor`.

## Cómo poner un video como fondo de pantalla en Linux

Instala Fresco, haz clic en **Agregar**, elige tu video y haz clic en **Establecer como fondo de pantalla**. El video se reproduce detrás de tus ventanas y vuelve solo en cada inicio de sesión.

1. Instala Fresco (arriba) y ábrelo desde el lanzador de aplicaciones.
2. Haz clic en **Agregar** y elige un video, GIF o imagen, o pega un enlace con **Desde enlace**.
3. Si quieres, recórtalo o gíralo en el editor.
4. Haz clic en **Establecer como fondo de pantalla** y cierra la aplicación.

El fondo de pantalla sigue reproduciéndose después de cerrar la ventana.

## Funciones

- **Cualquier medio**: video en bucle (mp4/webm/mkv), GIF animado, imagen fija, presentación de imágenes, lista de reproducción con varios videos
- **Agregar desde un enlace**: pega un pin de Pinterest o cualquier URL directa de video o imagen; Fresco la descarga y abre el editor de recorte
- **Vincular carpeta**: usa una carpeta de fondos de pantalla que ya tengas (imágenes, GIF y videos) sin importarla; se mantiene sincronizada cuando agregas y quitas archivos, las subcarpetas son opcionales y tus favoritos y ediciones se conservan por archivo
- **Bibliotecas grandes**: la biblioteca solo dibuja los fondos de pantalla cercanos a tu posición de desplazamiento, así que miles de fondos de pantalla se abren en unos dos segundos y usan unos cientos de megabytes (en una biblioteca de prueba con 4000 fondos de pantalla: unos 2 s y 325 MB, frente a entre 9 y 13 s y 1.6 GB)
- **Decodificación por hardware**: la decodificación de video en la GPU (VA-API / NVDEC) mantiene el uso de CPU casi en reposo al trasladar el trabajo al motor de video, donde es mensurablemente más barato (no es gratis; consulta [Rendimiento y duración de la batería](#rendimiento-y-duración-de-la-batería))
- **Ahorro de energía**: escalado más económico en la GPU para portátiles; medido, reduce a aproximadamente la mitad la potencia de la GPU (consulta [Rendimiento y duración de la batería](#rendimiento-y-duración-de-la-batería))
- **Varios monitores**: un fondo de pantalla distinto en cada pantalla, con reproducción sincronizada cuando se usa el mismo video en varios monitores
- **Horarios de día y noche**: cambia de fondo de pantalla con un temporizador, en franjas horarias a tu medida o según el amanecer y el atardecer
- **Widgets de escritorio**: letras sincronizadas, relojes con distintos temas, un reloj mundial, un visualizador de audio y un disco de portada de álbum que gira, dibujados en el propio fondo de pantalla para que nada tape tus ventanas (el reloj mundial tiene un modo opcional para flotar sobre las ventanas); todos desactivados por defecto (consulta [Widgets de escritorio](#widgets-de-escritorio-letras-relojes-visualizador-y-portada-del-álbum))
- **Pantalla de bloqueo**: muestra tu fondo de pantalla y tus widgets (reloj, saludo, lo que suena, batería y más) en tu pantalla de bloqueo real en lugar de la predeterminada: video en vivo y widgets en COSMIC 1.9+ y KDE Plasma 6, fondo de pantalla y widgets con `fresco lock` en Sway/Hyprland/niri y X11, y una imagen fija en el resto. Desactivada por defecto, y Fresco nunca ve tu contraseña; consulta [docs/LOCKSCREEN.md](docs/LOCKSCREEN.md)
- **Control con el teclado**: `fresco next`, `fresco prev` y `fresco random` para los atajos del compositor (una ventana de Fresco abierta recoge el cambio en lugar de revertirlo), y una paleta de comandos con Ctrl+K dentro de la aplicación
- **Gestión por lotes**: selecciona varios fondos de pantalla a la vez y quítalos en un solo paso
- **Catálogo integrado**: explora dentro de la aplicación fondos de pantalla seleccionados y con licencia en regla
- **Pausa automática a pantalla completa**: por monitor, en X11, compositores wlroots, COSMIC y KDE Plasma (mediante su complemento de fondo de pantalla); opcionalmente también con ventanas maximizadas, además de pausa con batería
- **Extensión para la pestaña nueva del navegador**: refleja tu fondo de pantalla en cada pestaña nueva (Chrome/Brave/Edge/Firefox; se carga sin empaquetar desde [`./extension`](extension))
- **Compatibilidad con Deepin DDE**: en Deepin 25, Fresco adapta el escritorio DDE automáticamente, y al hacer clic en el escritorio los iconos reaparecen durante diez segundos cuando los necesites (consulta [Solución de problemas](#mis-iconos-del-escritorio-están-ocultos-mientras-el-fondo-de-pantalla-se-reproduce-en-deepin))
- **Editor de recorte y rotación**, sonido y volumen por fondo de pantalla, transiciones de presentación y una biblioteca con búsqueda

## Entornos compatibles

| Entorno | Fondo de pantalla en vivo | Notas |
|---|---|---|
| X11 (GNOME, Cinnamon, XFCE, …) | ✅ | Renderizador integrado. GNOME solo donde la distribución aún incluye una sesión Xorg (consulta la nota sobre GNOME más abajo) |
| Kali Linux (Xfce, X11) | ✅ | Funciona en la sesión predeterminada de Kali, Xfce sobre X11, con el mismo renderizador integrado que los demás escritorios X11 |
| Xfce (X11) | ✅ | Los iconos del escritorio siguen visibles sobre el fondo de pantalla, y puedes pulsarlos, seleccionarlos y arrastrarlos |
| MATE (X11) | ✅ | Los iconos del escritorio siguen visibles y se pueden pulsar sobre el fondo de pantalla |
| Deepin 25 (DDE, X11) | ✅ | Adaptación automática a DDE; verificada por la comunidad en Deepin 25 Community build1 |
| COSMIC (Wayland) | ✅ | layer-shell |
| Hyprland | ✅ | layer-shell |
| Sway | ✅ | layer-shell |
| Cinnamon 6.8 y posteriores (Wayland) | ✅ | El compositor de Cinnamon incorporó layer-shell en la 6.8. Cinnamon 6.6 y anteriores no tienen layer-shell y muestran una imagen fija en Wayland |
| KDE Plasma 6 (X11 y Wayland) | ✅ | Se establece mediante el propio complemento de fondo de pantalla de plasmashell, por lo que los iconos del escritorio siguen visibles. Necesita `qml6-module-qtmultimedia` para video. La reproducción es de Qt, no de mpv: sin sonido, sin ajuste de hwdec, recorte ni transiciones; una lista de reproducción reproduce su primer archivo y una presentación muestra su primer fotograma. Se respeta la opción Ajuste (Rellenar, Ajustar, Estirar) |
| GNOME en Wayland | ⚠️ | Solo imagen fija: Mutter no ofrece ninguna superficie para fondos de pantalla en vivo, así que no hay video ni widgets. El video en vivo requiere una extensión de GNOME para Fresco (planificada) |

Todos los entornos anteriores se prueban en modo headless en la CI en cada versión.

**Nota sobre GNOME.** GNOME en Wayland es el único escritorio mayoritario en el que Fresco solo puede mostrar una imagen fija. La vieja solución, iniciar sesión en una sesión Xorg, está desapareciendo: GNOME 49 desactivó su sesión X11 y GNOME 50 la eliminó, de modo que Ubuntu 25.10 y posteriores (incluida la 26.04 LTS) y Fedora 43 y posteriores no incluyen ninguna sesión Xorg a la que cambiar. Ubuntu 22.04 y 24.04 siguen ofreciendo «Ubuntu on Xorg». Donde no hay sesión Xorg, el video en vivo en el escritorio GNOME estándar tendrá que esperar a una extensión de GNOME para Fresco, que está planificada; Fresco lo dice en la aplicación en lugar de fingir que el video se está reproduciendo. KDE Plasma, COSMIC, Hyprland, Sway y todos los escritorios X11 no se ven afectados.

> «Fácil de usar y con una interfaz limpia: una de las pocas aplicaciones de fondos de pantalla en vivo realmente adaptadas a Deepin 25, que se instala con un .deb y funciona con fluidez gracias a la reproducción acelerada por hardware».
>
> — 柒玖 (foro de deepin) / 柒仈玖 (GitHub), probado en Deepin 25 Community build1, sesión X11, Intel Alder Lake-N [Intel Graphics]

Deepin 25 incluye X11 como sesión predeterminada, y esa es la sesión en la que Fresco está verificado allí. El compositor Wayland propio de Deepin, [Treeland](https://github.com/linuxdeepin/treeland), sigue en desarrollo, por lo que Fresco todavía no afirma nada sobre Deepin en Wayland.

## Fresco frente a Wallpaper Engine, Hidamari, Komorebi y mpvpaper

| Herramienta | Plataforma | Wayland | X11 | Interfaz | Reproduce | Licencia |
|---|---|---|---|---|---|---|
| **Fresco** | Linux | ✅ compositores con layer-shell y KDE Plasma 6; imagen fija en GNOME | ✅ | Aplicación GTK4 y una CLI `fresco` | Video, GIF, imágenes, presentaciones, listas de reproducción | GPL-3.0-or-later |
| **Wallpaper Engine** | Windows (Steam); sin versión oficial para Linux | n/d | n/d | Aplicación gráfica | Fondos de video, de escena y web | De pago, código cerrado |
| **[linux-wallpaperengine](https://github.com/Almamu/linux-wallpaperengine)** | Linux; necesita los recursos de una copia de Wallpaper Engine en Steam | ✅ compositores con layer-shell | ⚠️ no cuando un entorno de escritorio (GNOME, KDE) dibuja el fondo | Línea de comandos; su README enumera interfaces gráficas de la comunidad por separado | Fondos de Wallpaper Engine | GPL-3.0 |
| **[Hidamari](https://github.com/jeffshee/hidamari)** | Linux, orientado a GNOME | ⚠️ GNOME Wayland | ✅ | Aplicación gráfica | Video | GPL-3.0 |
| **[Komorebi](https://github.com/cheesecakeufo/komorebi)** | Linux | — | — | Aplicación gráfica | Fondos de imagen, video y página web | GPL-3.0 |
| **[mpvpaper](https://github.com/GhostNaN/mpvpaper)** | Linux, compositores basados en wlroots | ✅ layer-shell | ❌ | Línea de comandos | Video, mediante mpv | GPL-3.0 |
| **xwinwrap** | Linux, solo X11 | ❌ | ✅ | Línea de comandos | Lo que le conectes como reproductor (mpv, gifview) | — |
| **swww** (ahora [awww](https://codeberg.org/LGFae/awww)) | Linux, compositores con layer-shell; no GNOME | ✅ | ❌ | Demonio de línea de comandos | GIF animados e imágenes, no video | GPL-3.0 |

Fresco incluye `mpvpaper` como su renderizador para Wayland, así que se apoya en ese proyecto en lugar de competir con él. Un `—` significa que la documentación del proyecto no lo aclara. Comprobado en el repositorio público de cada proyecto el 2026-10-16: el último push al repositorio de Komorebi fue en julio de 2025, y el repositorio de swww en GitHub está archivado porque el proyecto se mudó a Codeberg con el nombre awww.

## Rendimiento y duración de la batería

柒玖 (foro de deepin) / 柒仈玖 (GitHub) midió la potencia del paquete con `turbostat` en un Intel N150 (Deepin 25, VA-API, dos ejecuciones por nivel) mientras se reproducía un fondo de pantalla de video:

| Video | Ahorro de energía | Potencia de la GPU | Potencia total del paquete |
|---|---|---|---|
| 1080p 60fps | Calidad completa | 1.37 W | 6.00 W |
| 1080p 60fps | **Reducida** (predeterminada) | **0.63 W** (−54 %) | **4.03 W** (−33 %) |
| 4K 60fps | Calidad completa | 2.77 W | 7.94 W |
| 4K 60fps | **Reducida** (predeterminada) | **1.60 W** (−42 %) | **5.95 W** (−25 %) |
| 4K 60fps | Mínima | 0.99 W (−65 %) | 4.97 W (−37 %) |

El ahorro de energía reduce el costo de escalado por fotograma en la GPU. No se descarta ningún fotograma y la decodificación por hardware no se toca, así que la reproducción sigue siendo fluida: lo que se sacrifica es la nitidez de la imagen, no el movimiento. Reducida es el valor predeterminado; con fuentes 4K conviene elegir Mínima.

**Cómo leer estas cifras.** La decodificación por hardware no hace que un fondo de pantalla en vivo sea gratis: traslada el trabajo de la CPU al motor de video, donde cuesta menos, pero no desaparece. «Poco uso de CPU» por sí solo es una afirmación débil, porque la alternativa con la que se compara es la decodificación por software, que es mucho peor; la medida honesta es el consumo de todo el sistema, y por eso la tabla indica la **potencia total del paquete** y no solo la de la GPU.

Dos salvedades sobre las cifras anteriores, para que no se lean como algo más de lo que son:

- Son la **potencia total del paquete mientras se reproduce un fondo de pantalla**, no el costo marginal del fondo de pantalla. No se registró una línea base en reposo en la misma máquina, así que aquí no se establece la diferencia entre estas cifras y un escritorio inactivo. Los porcentajes comparan entre sí los *niveles* de ahorro de energía, y para eso sí son válidos.
- Son de una sola máquina (Intel N150, Alder Lake-N, VA-API, Deepin 25, dos ejecuciones por nivel). Las GPU dedicadas, NVDEC y otros controladores darán resultados distintos.

Un fondo de pantalla en vivo siempre cuesta más que uno estático. El ahorro de energía, la pausa automática a pantalla completa y la pausa con batería existen para acotar ese costo, no para fingir que es cero.

## Preguntas frecuentes (FAQ)

### ¿Existe una alternativa a Wallpaper Engine para Linux?

Sí. Fresco es una aplicación gratuita y de código abierto para Linux que reproduce tus propios fondos de pantalla de video, GIF e imagen mediante una interfaz gráfica, con decodificación por hardware y sin capa de compatibilidad. No abre los archivos de fondos propios de Wallpaper Engine; si tienes Wallpaper Engine y quieres usar esos fondos en Linux, el proyecto independiente [linux-wallpaperengine](https://github.com/Almamu/linux-wallpaperengine) los ejecuta desde la línea de comandos y necesita los recursos de una copia de Wallpaper Engine en Steam. Consulta la [tabla comparativa](#fresco-frente-a-wallpaper-engine-hidamari-komorebi-y-mpvpaper).

### ¿Funciona Wallpaper Engine en Linux?

No de forma nativa. Wallpaper Engine es una aplicación de Windows sin versión oficial para Linux. Fresco es una alternativa nativa para Linux que se instala como `.deb` y no necesita ninguna capa de compatibilidad.

### ¿Cómo pongo un video como fondo de pantalla animado en Ubuntu?

Instala Fresco, ábrelo, haz clic en **Agregar**, elige tu video y haz clic en **Establecer como fondo de pantalla**. El video se reproduce como fondo del escritorio y se restaura al iniciar sesión. La sesión predeterminada de Ubuntu (GNOME en Wayland) solo puede mostrar una imagen fija. En Ubuntu 22.04 y 24.04 puedes iniciar sesión en **Ubuntu on Xorg** para tener la reproducción en vivo completa; Ubuntu 25.10 y posteriores (incluida la 26.04 LTS) no incluyen sesión Xorg, así que allí el video en vivo en el escritorio GNOME estándar requiere una extensión de GNOME para Fresco, que está planificada. Las variantes de Ubuntu que usan KDE Plasma, Xfce o MATE no se ven afectadas.

### ¿Cómo consigo un fondo de pantalla en vivo en Ubuntu, Linux Mint, Pop!_OS, Fedora o Arch Linux?

Instala el `.deb` de Fresco en Ubuntu, Linux Mint y Pop!_OS, o compílalo desde el código fuente en Fedora y Arch Linux. Los comandos de cada caso están en [Instalación](#instalación).

- **Ubuntu:** `.deb`. En el escritorio GNOME predeterminado, la reproducción en vivo necesita una sesión Xorg (Ubuntu 22.04 y 24.04); las variantes con KDE Plasma, Xfce y MATE no se ven afectadas. Consulta la respuesta sobre Ubuntu más arriba.
- **Linux Mint:** `.deb`. Se admite Cinnamon en X11 y Cinnamon en Wayland desde la versión 6.8.
- **Pop!_OS:** `.deb`. Se admite el escritorio COSMIC (Wayland, layer-shell).
- **Fedora:** compilar desde el código fuente. GNOME en Fedora 43 y posteriores es solo Wayland, así que muestra una imagen fija; KDE Plasma y los escritorios X11 son compatibles.
- **Arch Linux:** compilar desde el código fuente e instalar `mpvpaper` desde el AUR (`yay -S mpvpaper`).

Solo la vía del `.deb` está cubierta por la CI de Fresco.

### ¿Qué distribuciones de Linux son compatibles?

Fresco incluye un paquete `.deb` para las distribuciones basadas en Debian y Ubuntu: Ubuntu, Pop!_OS, Linux Mint, Debian, elementary OS, Deepin 25 y Kali Linux. Las demás distribuciones pueden compilarlo desde el código fuente; consulta [Instalación](#instalación) y [docs/INSTALL.md](docs/INSTALL.md).

### ¿Los fondos de pantalla en vivo consumen mucha CPU, GPU o batería?

CPU, no. Batería, algo: un fondo de pantalla en vivo nunca es gratis.

Fresco decodifica el video en la GPU (VA-API / NVDEC), por lo que el uso de CPU se mantiene casi en reposo. Eso traslada el costo al motor de video en lugar de eliminarlo. Medido en un Intel N150 con el nivel predeterminado de ahorro de energía, un fondo de pantalla de 1080p consumió 0.63 W de potencia de GPU, con una potencia total del paquete de 4.03 W durante la reproducción (en [Rendimiento y duración de la batería](#rendimiento-y-duración-de-la-batería) se explica cómo leer esa cifra, incluido lo que *no* dice). La pausa automática a pantalla completa y la pausa con batería existen para que ese costo no recaiga sobre tu batería cuando más importa.

### ¿Funciona Fresco en Wayland?

Sí, en los compositores que implementan el protocolo layer-shell (COSMIC, Hyprland y Sway) y en KDE Plasma 6 (X11 y Wayland) mediante el propio complemento de fondo de pantalla de plasmashell. GNOME en Wayland es la excepción: Mutter no ofrece ninguna superficie para el fondo de pantalla, así que Fresco muestra una imagen fija y lo indica en la aplicación. Las sesiones X11 son totalmente compatibles.

### ¿Funciona Fresco en GNOME?

Sí en X11, con el fondo de pantalla en vivo completo: video, widgets, todo. En **GNOME con Wayland**, Mutter no expone ninguna superficie de fondo de pantalla en la que una aplicación pueda dibujar, así que Fresco muestra una imagen fija y los widgets no están disponibles. Si tu distribución aún incluye una sesión Xorg (Ubuntu 22.04 y 24.04 la incluyen), cierra sesión y elige «GNOME on Xorg» o «Ubuntu on Xorg» para tener la reproducción en vivo completa. GNOME 49 desactivó esa sesión y GNOME 50 la eliminó, así que Ubuntu 25.10 y posteriores (incluida la 26.04 LTS) y Fedora 43 y posteriores no la tienen: en ellos, el video en vivo en GNOME requiere una extensión de GNOME para Fresco, que está planificada. `fresco doctor` te dice en cuál de los casos estás.

### ¿Funciona Fresco en KDE Plasma, COSMIC, Hyprland o Sway?

Sí, en los cuatro. COSMIC, Hyprland y Sway reproducen video en vivo mediante layer-shell. KDE Plasma 6 (X11 y Wayland) usa el propio complemento de fondo de pantalla de Fresco dentro de plasmashell, que mantiene visibles los iconos del escritorio pero reproduce con Qt en lugar de mpv: sin sonido, sin recorte ni transiciones, y necesita `qml6-module-qtmultimedia` para video. La opción Ajuste (Rellenar, Ajustar o Estirar) también se aplica ahí.

### ¿Funciona Fresco en Linux Mint, Xfce, Cinnamon o MATE?

Sí. Cinnamon, Xfce y MATE en X11 usan el renderizador integrado, y en Xfce y MATE los iconos del escritorio siguen visibles sobre el fondo de pantalla; en Xfce puedes pulsarlos, seleccionarlos y arrastrarlos como siempre. Cinnamon en Wayland funciona desde Cinnamon 6.8, que incorporó layer-shell.

### ¿Cómo cambio de fondo de pantalla con un atajo de teclado?

Asigna `fresco next`, `fresco prev` o `fresco random` a una tecla en tu compositor o en los ajustes del escritorio: cambian el fondo de pantalla sin abrir la aplicación (`random` nunca elige el que ya se está reproduciendo). Recorren tu biblioteca en el orden en que la muestra la aplicación, o solo la carpeta que tengas abierta allí, e inician el servicio de Fresco si no está en ejecución. Si la ventana de Fresco está abierta en ese momento, recoge el fondo de pantalla nuevo en lugar de volver a poner el anterior. Dentro de la aplicación, Ctrl+K abre una paleta de comandos. Por ejemplo, en Sway:

```
bindsym $mod+Right  exec fresco next
bindsym $mod+Left   exec fresco prev
bindsym $mod+Return exec fresco random
```

### ¿Puedo usar un fondo de pantalla distinto en cada monitor?

Sí. Fresco admite un fondo de pantalla por pantalla, y cuando se usa el mismo video en varios monitores, la reproducción se mantiene sincronizada.

### ¿Admite GIF y presentaciones de imágenes?

Sí: GIF animados, imágenes estáticas, presentaciones de imágenes con transiciones (desvanecer a negro, Ken Burns, acercamiento y desenfoque) y listas de reproducción con varios videos, además de archivos de video.

### ¿Puede Fresco usar mi carpeta de fondos de pantalla sin importarla?

Sí. Elige **Vincular carpeta…** en el menú (o **Vincular esta carpeta** después de **Agregar carpeta**) para indicarle a Fresco una carpeta donde ya guardas fondos de pantalla. Sus imágenes, GIF y videos aparecen en tu biblioteca, en una sección propia, donde están y sin importarse, y la biblioteca se mantiene sincronizada cuando agregas y quitas archivos en esa carpeta. **Incluir subcarpetas** es opcional (hasta cuatro niveles de profundidad), y tus favoritos y ediciones se recuerdan por archivo. Si la carpeta deja de estar disponible, por ejemplo por desconectar una unidad, su sección sigue marcada como «(no disponible)» y vuelve sola. **Desvincular carpeta**, en el menú de la sección, quita la sección sin borrar ningún archivo. **Agregar** sigue importando archivos sueltos como antes.

### ¿Puede Fresco manejar miles de fondos de pantalla?

Sí. La biblioteca solo construye las tarjetas de los fondos de pantalla cercanos a donde estás desplazándote, en lugar de todos a la vez, así que una colección grande no sobrecarga la aplicación. En una biblioteca de prueba con 4000 fondos de pantalla (mitad imágenes, mitad videos), la biblioteca apareció en unos 2 segundos en lugar de entre 9 y 13, y la aplicación usó unos 325 MB de memoria en lugar de 1.6 GB. Esas cifras provienen de una compilación de desarrollo bajo carga en una sola máquina, así que tómalas como una guía, no como una promesa.

### ¿Puedo usar un video de YouTube como fondo de pantalla?

No pegando su enlace. **Desde enlace** acepta pines de Pinterest y enlaces directos a un archivo de video o imagen (mp4, webm, mkv, mov, gif, jpg, png, webp); los demás enlaces se rechazan con un mensaje. Agrega con **Agregar** un archivo de video que ya tengas, o pega un enlace directo al archivo.

### ¿Fresco es gratuito y de código abierto?

Sí. Fresco es gratuito y de código abierto bajo GPL-3.0-or-later. No hay anuncios, ni cuentas, ni versión de pago.

### ¿Fresco recopila mis datos?

No se envía nada hasta que respondes a un cuadro de diálogo de consentimiento que aparece una sola vez. Eliges entre un recuento diario anónimo (un identificador de instalación aleatorio, el país y la versión de la aplicación) y ese mismo recuento más detalles opcionales; con `telemetry = false` en `~/.config/fresco/config.toml` no se envía absolutamente nada. Consulta [Privacidad y condiciones de uso](#privacidad-y-condiciones-de-uso) para ver todos los campos.

### ¿Cuál es la diferencia entre Fresco y mpvpaper?

`mpvpaper` es un envoltorio de mpv para la línea de comandos que reproduce un solo video como fondo layer-shell de Wayland: sin interfaz gráfica, sin biblioteca y sin compatibilidad con X11. Fresco es una aplicación de escritorio GTK4 construida a su alrededor: con una biblioteca de fondos de pantalla, editor de recorte y rotación, programación, sincronización entre varios monitores, widgets de escritorio y compatibilidad con X11, sin necesidad de terminal. Fresco incluye `mpvpaper` como su renderizador para Wayland en lugar de reemplazarlo; consulta la [tabla comparativa](#fresco-frente-a-wallpaper-engine-hidamari-komorebi-y-mpvpaper).

### ¿Puedo usar Fresco en Kali Linux?

Sí. Kali Linux está basada en Debian, así que Fresco se instala igual que en Debian o Ubuntu: con el paquete `.deb` o con el script de instalación de una línea (consulta [Instalación](#instalación)). El escritorio predeterminado de Kali, Xfce sobre X11, es compatible gracias al renderizador X11 integrado de Fresco.

### ¿Cómo quito varios fondos de pantalla a la vez?

Haz clic en **Seleccionar** en el pie (o haz clic derecho en un fondo de pantalla y elige **Seleccionar…**), marca los que quieras y haz clic en **Quitar**. **Seleccionar todo** respeta la búsqueda actual, así que puedes buscar primero y luego quitar todo un lote. Quitar un fondo de pantalla solo lo saca de tu biblioteca de Fresco: el archivo original en el disco se conserva.

### ¿Cómo desinstalo Fresco?

Si instalaste el `.deb` o usaste el script de instalación, ejecuta `sudo apt remove fresco`. Tu biblioteca y tu configuración están en `~/.local/share/fresco/` y `~/.config/fresco/`; elimina esos directorios para borrar todo rastro. Para recuperar el fondo de pantalla propio de tu escritorio sin desinstalar, abre Fresco y haz clic en **Detener fondo de pantalla**.

## Widgets de escritorio: letras, relojes, visualizador y portada del álbum

Fresco dibuja cinco widgets opcionales sobre tu fondo de pantalla. Todos están **desactivados por defecto**; actívalos desde el menú de la aplicación (Ctrl+,) → **Avanzado…**.

| Widget | Qué obtienes |
|---|---|
| **Letras** | La línea actual, al compás de la música. Cuatro preajustes (Minimalista, Karaoke, Subtítulo, Tarjeta), una cuadrícula de colocación de nueve puntos, un control deslizante de desfase de sincronización, una línea siguiente atenuada opcional y el título y el artista de la pista, también opcionales |
| **Reloj** | Ocho temas: Digital, Minimalista, Segmentos, Apilado, Detallado, Tarjeta, NOS y una esfera al estilo del teléfono, Pantalla de bloqueo. Formato de 12 o 24 horas, fecha opcional y controles de color y opacidad. Los segundos están desactivados por defecto porque cuestan 60 veces más repintados |
| **Reloj mundial** | La hora en varias ciudades a la vez, elegidas de la lista completa de zonas horarias, como texto sencillo de 12 horas (por ejemplo `Tokyo  9:41 PM`) con una etiqueta opcional por ciudad. Se repinta una vez por minuto. Novedad en la 1.1.50 |
| **Visualizador de audio** | Cinco estilos (Barras, Reflejar, Onda, Puntos, Anillo) con selector de color, degradado entre dos colores o arcoíris |
| **Portada del álbum** | La portada de la pista actual sobre un disco que gira; deja de girar cuando se pausa la reproducción |

Salvo el modo opcional **Flotar sobre las ventanas** del reloj mundial, los widgets se pintan en el fondo de pantalla mediante la capa OSD de mpv en lugar de en una ventana propia, así que nunca quedan por encima de tus ventanas, nunca interceptan un clic y se comportan igual en X11 y en todos los compositores con layer-shell. Con música sonando y los widgets de letras, reloj, visualizador y portada del álbum activados, el costo medido fue del **0.8 % de un núcleo de CPU**, casi todo por la captura de audio, ya que nada se repinta a menos que cambie su contenido. Los widgets también vuelven por sí solos cuando el equipo se suspende y se reanuda.

Los widgets aparecen por defecto en **todas las pantallas**. Para dejarlos en una sola, añade `monitor = "DP-1"` al bloque `[widgets]` de `config.toml` (todavía no hay un control en la interfaz gráfica para esto). **No están disponibles en GNOME con Wayland**, que no tiene ninguna superficie de fondo de pantalla en vivo en la que Fresco pueda dibujar; es la misma razón por la que allí los fondos de pantalla vuelven a una imagen fija.

### ¿Puedo añadir un reloj al fondo de pantalla de mi escritorio Linux?

Sí. Activa el widget **Reloj** para tener un reloj con tema dibujado en el fondo de pantalla, o el **Reloj mundial** para mostrar varias ciudades a la vez. El reloj mundial se sitúa sobre el fondo de pantalla como los demás widgets; su modo opcional **Flotar sobre las ventanas** lo eleva por encima de todas las aplicaciones y deja pasar los clics, algo que funciona en X11 con compositor y en compositores con layer-shell (KDE Plasma, COSMIC, Sway, Hyprland), y vuelve al fondo de pantalla donde una sesión no puede hacerlo flotar, como GNOME con Wayland.

### ¿Puedo mostrar la letra de las canciones en mi escritorio Linux?

Sí. Fresco dibuja en tu fondo de pantalla **letras sincronizadas con el tiempo**, siguiendo lo que esté sonando en tu sistema a través de MPRIS: navegadores, reproductores de música, reproductores de video. Elige un preajuste, la colocación y el desfase de sincronización en los ajustes del widget Letras (consulta la tabla de arriba).

### ¿Linux tiene widgets de escritorio como Conky?

Sí, y Fresco añade cinco que no necesitan panel, ni extensión, ni ayuda de tu escritorio: un reloj, un reloj mundial, letras sincronizadas, un visualizador de audio y un disco de portada de álbum que gira. Como se pintan en el fondo de pantalla en lugar de en una ventana, funcionan en escritorios que no tienen capa de widgets propia, incluidos COSMIC, Hyprland y Sway. A diferencia de Conky, Fresco **no tiene widgets de monitorización del sistema** (ni lecturas de CPU, RAM, temperatura o red), de modo que acompaña a Conky como complemento de música y hora en lugar de reemplazarlo. GNOME con Wayland es el único lugar donde no puede funcionar.

### ¿Puedo tener un visualizador de música en el fondo del escritorio?

Sí. El visualizador de audio de Fresco reacciona a lo que esté reproduciendo tu sistema, en uno de cinco estilos (Barras, Reflejar, Onda, Puntos o Anillo), con selector de color, degradado entre dos colores o arcoíris. Está desactivado por defecto y pide consentimiento la primera vez que lo activas, porque tiene que escuchar la salida de audio; el consentimiento también se comprueba al cargar la configuración, de modo que editar `config.toml` a mano no puede activarlo a tus espaldas. Combínalo con el widget de portada del álbum para tener un disco giratorio con la portada de la pista actual.

### ¿Qué reproductores de música funcionan con el widget de letras?

Cualquiera que publique metadatos MPRIS estándar, aunque no todos son igual de fiables:

| Reproductor | ¿Funciona? | Notas |
|---|---|---|
| **Firefox** | ✅ | La opción fiable, y con la que se verificó la función |
| Chrome / Brave / Edge / Vivaldi / Opera | ⚠️ | Consulta más abajo |
| Spotify, **en un navegador** | ✅ | Informa correctamente de la posición de reproducción |
| Spotify, **cliente nativo para Linux** | ⚠️ | Informa su posición siempre como 0, por lo que las letras no pueden mantenerse sincronizadas |
| Reproductores locales (VLC, mpv, Rhythmbox, …) | ✅ | También es el caso en el que es más probable que exista un archivo `.lrc` local |

Los navegadores de la familia Chromium reservan un nombre MPRIS la primera vez
que cualquier pestaña reproduce contenido multimedia y **nunca lo liberan**, un
error de Chromium que viene de lejos. Cuando termina la reproducción, el título
se borra, pero queda en el bus una sesión «zombi» obsoleta, a veces todavía con
la carátula. Fresco ignora cualquier reproductor que no publique un título de
pista, lo que descarta esas sesiones, pero la información de Chromium sigue
siendo tan inconsistente que Firefox es el navegador que conviene usar para esto.

El cliente nativo de Spotify para Linux devuelve `Position: 0` y no emite ningún
seek desde 2018, tanto en las compilaciones nativas como en las de snap. Fresco
lo detecta por su comportamiento (tres lecturas de cero espaciadas durante la
reproducción) y deja correr libremente el reloj de la letra desde el cambio de
pista, lo que se desfasa si saltas por la canción. Spotify en un navegador no
tiene ese problema.

### ¿De dónde salen las letras?

Primero de un archivo `.lrc` local: uno junto al archivo de audio, o uno que
coincida en una carpeta de letras que le indiques a Fresco. Esa vía funciona sin
conexión y es la mejor coincidencia, porque es el archivo que tú elegiste.

Cuando no hay archivo local, que es lo más habitual si usas streaming, Fresco
busca la pista en [LRCLIB](https://lrclib.net), una base de datos gratuita de
letras sincronizadas mantenida por la comunidad, y guarda la respuesta en caché
en `~/.cache/fresco/lyrics`, de modo que la misma canción no se descarga dos
veces. Esa consulta envía a LRCLIB el título, el artista y el álbum de la pista;
no se envía nada a menos que el widget de letras esté activado y la pista no
tenga archivo local. Fresco no aloja, no posee ni licencia contenido de letras:
las entradas de LRCLIB las aportan sus usuarios y LRCLIB no declara ninguna
licencia sobre ellas, por lo que Fresco las obtiene bajo demanda y las guarda en
caché por usuario en lugar de distribuir su propia base de datos de letras.

Los tiempos de las letras en los archivos `.lrc` se hacen a mano y a menudo
están algo desfasados. Para eso existe un control deslizante de desfase de
sincronización en los ajustes de letras.

## Solución de problemas

Ejecuta primero `fresco doctor`: informa de tu sesión, de qué renderizador eligió Fresco y de dónde salió, y de si se aplica la solución alternativa de Xorg en GNOME. [docs/INSTALL.md](docs/INSTALL.md) cubre más casos, incluido el uso elevado de CPU. Incluye la salida de `fresco doctor` cuando abras un issue.

¿Sigues atascado? [Informa de un error](https://github.com/DibbayajyotiRoy/Fresco/issues/new?template=bug_report.yml) o [solicita una función](https://github.com/DibbayajyotiRoy/Fresco/issues/new?template=feature_request.yml).

### Fresco no aparece en el lanzador de Deepin después de instalarlo: ¿por qué?

Es un problema conocido de dde-launchpad: su actualización posterior a la
instalación no detecta Fresco. Ejecuta `killall dde-shell` (se reinicia solo) o
cierra sesión y vuelve a entrar, y la entrada aparecerá de forma permanente.
Fresco se instala correctamente: figura en el administrador de aplicaciones de
Deepin y su icono se resuelve en todos los temas instalados. Si ocurre en cada
reinstalación en un equipo que ya tuvo Fresco, ejecuta
`sh scripts/dde-launcher-diag.sh`: es de solo lectura e imprime un registro que
conviene adjuntar a un issue.

### Mis iconos del escritorio están ocultos mientras el fondo de pantalla se reproduce en Deepin

Deepin 25 dibuja su fondo de pantalla y sus iconos del escritorio en una única
ventana opaca, así que un fondo de pantalla en vivo que se vea es uno colocado
por encima de esa ventana: no hay ninguna capa que quede entre ambos. **Haz clic
en el escritorio y los iconos vuelven durante diez segundos**, lo suficiente para
abrir lo que buscabas; después regresa el fondo de pantalla, y otro clic compra
otros diez segundos. Para cambiar el tiempo, define `dde_icon_peek_secs` en
`~/.config/fresco/config.toml` (`0` mantiene siempre el fondo de pantalla por
encima). El fondo de pantalla propio de Deepin se sigue mostrando siempre que
Fresco esté apagado o en pausa.

### ¿Por qué el fondo de mi escritorio MATE se ve oscuro después de un fallo?

Para mantener los iconos del escritorio de Caja por encima del fondo de pantalla
en vivo, Fresco establece el fondo del escritorio de MATE en un color clave casi
negro (`#010101`) mientras se ejecuta y copia sobre el fondo de pantalla todo lo
que no sea ese color, es decir, los iconos. Tu propio fondo se guarda antes y se
restaura cuando Fresco se detiene o se desactiva. Si Fresco falla antes de poder
hacerlo, el escritorio se queda oscuro; **vuelve a iniciar Fresco y restaurará
tu fondo**. En un servidor X sin las extensiones Composite y Damage, Fresco
funciona como en [Deepin](#mis-iconos-del-escritorio-están-ocultos-mientras-el-fondo-de-pantalla-se-reproduce-en-deepin):
iconos ocultos, y un clic en el escritorio los hace volver.

### Mi fondo de pantalla se ve negro en Wayland (NVIDIA, COSMIC, Hyprland, Sway)

Un fondo de pantalla negro, con todo lo demás funcionando y sin errores en el
registro, casi siempre significa que el renderizador mpvpaper que incluye Fresco
es demasiado antiguo para tu configuración. Las versiones anteriores a la 1.6
inicializan EGL, informan de que todo fue bien y después nunca presentan un
fotograma con el controlador propietario de NVIDIA.

Ejecuta primero `fresco doctor`. Muestra el renderizador que eligió, de dónde
salió ese binario (incluido o instalado por ti) y aproximadamente qué versión es,
y avisa cuando esa versión es anterior a la corrección:

```
  ✓ mpvpaper available (/usr/lib/fresco/mpvpaper-libmpv2)
      source: bundled · version: 1.4–1.6
  ⚠ mpvpaper may be too old …
```

Fresco ya prefiere un `mpvpaper` más reciente en tu `PATH` frente a uno incluido
más antiguo, así que normalmente basta con instalar el paquete `mpvpaper` de tu
distribución (o compilar el [proyecto original](https://github.com/GhostNaN/mpvpaper)
con `scripts/build-mpvpaper.sh`). Para indicarle a Fresco un binario concreto,
define **`FRESCO_MPVPAPER`** con su ruta completa: esto anula cualquier otra
elección:

```bash
mkdir -p ~/.config/environment.d
echo 'FRESCO_MPVPAPER=/home/YOU/.local/bin/mpvpaper' > ~/.config/environment.d/fresco.conf
```

Cierra sesión y vuelve a entrar (systemd lee `environment.d` al iniciar la
sesión) y luego confirma con `fresco doctor` que la línea `source:` ahora dice
`FRESCO_MPVPAPER override`.

De forma parecida, **`FRESCO_HWDEC`** anula el decodificador por hardware de mpv
(p. ej. `nvdec`, `nvdec-copy`, `vaapi`, `no`), un recurso de diagnóstico para el
uso elevado de CPU. Por defecto, Fresco elige `nvdec,vaapi,auto-safe` cuando hay
una GPU NVIDIA y `auto-safe` en caso contrario (el video rotado usa copy-back:
`nvdec-copy,auto-copy` / `auto-copy`).

## Privacidad y condiciones de uso

**No se envía nada hasta que respondes.** Un cuadro de diálogo de consentimiento pregunta una sola vez en el primer inicio, y la elección se puede cambiar en cualquier momento en Configuración.

En ambos casos, una vez al día Fresco registra que una instalación estuvo activa, en qué país y con qué versión. Eso es el recuento. Lo que pregunta el cuadro de diálogo son los **detalles**:

| Tu elección | Qué envía Fresco |
| --- | --- |
| **Aceptar todo** | El recuento, además de la distribución, el escritorio, el tipo de sesión, el backend de video, el número de monitores, qué funciones usas, los tipos de error, la ciudad y la región, y la hora exacta de cada registro. |
| **Rechazar lo opcional** | Solo el recuento: un identificador de instalación aleatorio (que nunca se deriva de tu hardware ni de tu nombre), el país, la versión de la aplicación y el tipo de paquete. Tu registro se guarda como una **fecha, no como una hora**. |

**La ciudad y la hora exacta de uso son opcionales**: solo se envían si aceptas todo y nunca se envían si lo rechazas. Las coordenadas no se recopilan en ningún nivel: el punto de acceso de geolocalización descarta la latitud y la longitud en lugar de devolverlas.

Estas cifras sirven para decidir qué distribuciones y escritorios se prueban antes de una versión y dónde las descargas necesitan un espejo. Nunca se venden, se comparten ni se usan para publicidad, y no interviene ningún proveedor de analítica.

**No se recopila nunca, en ninguno de los dos casos:** datos personales, nombres de archivo, tus fondos de pantalla, tu dirección IP, audio, pulsaciones de teclas ni el contenido del portapapeles. El país lo resuelve Cloudflare a partir de tu IP en el borde de la red, de modo que a este proyecto solo llega un código de dos letras. (El texto que escribes deliberadamente y envías, como un comentario de opinión o un mensaje al mantenedor que aparece más abajo, es un mensaje que elegiste enviar, no una recopilación de datos.)

Para no enviar absolutamente nada, define `telemetry = false` y `telemetry_prompted = false` en `~/.config/fresco/config.toml`.

### Habla con el mantenedor, de forma anónima

**Menú → Escribir al mantenedor** abre una conversación privada, en ambos sentidos, con la persona que hace Fresco. Sin cuenta, sin dirección de correo, sin inicio de sesión de GitHub. Es anónima en las dos direcciones: tú no sabes de esa persona más que «el mantenedor», y ella no sabe quién eres, solo recibe tus mensajes y, si dejas la casilla marcada, el resumen de tu configuración que se muestra en el cuadro de diálogo.

La conversación se identifica con un ticket aleatorio que se genera por separado de la telemetría y se guarda en un archivo distinto, de modo que una conversación nunca puede vincularse con un perfil de uso. Funciona exactamente igual si aceptaste todo o rechazaste lo opcional. No existe nada hasta que envías el primer mensaje.

📄 **[Condiciones de uso completas y política de privacidad →](TERMS.md)**: cada campo, en una tabla, sin omitir nada. Todas las líneas que envían algo están en [`src/telemetry.rs`](src/telemetry.rs), así que puedes comprobarlo en lugar de confiar.

## Datos del proyecto

| | |
|---|---|
| **Nombre** | Fresco (ID de aplicación `io.github.dibbayajyotiroy.Fresco`) |
| **Qué es** | Una aplicación de fondos de pantalla en vivo y de video para el escritorio Linux |
| **Licencia** | [GPL-3.0-or-later](LICENSE): libre y de código abierto, sin anuncios y sin cuenta |
| **Lenguaje y tecnologías** | Rust, GTK4 / libadwaita, libmpv; un renderizador mpvpaper incluido en Wayland |
| **Primera versión** | v0.0.1, junio de 2026 |
| **Última versión** | 1.1.50 (2026-10-16); consulta [Releases](https://github.com/DibbayajyotiRoy/fresco/releases/latest) para ver la más reciente |
| **Paquetes** | `.deb` (amd64), script de instalación de una línea, App Store de Deepin; compilación desde el código fuente en el resto |
| **Idiomas de la interfaz** | Inglés más 12 traducciones: chino simplificado, ruso, español, francés, alemán, portugués (Brasil), turco, vietnamita, indonesio, hindi, bengalí y rumano |
| **Usuarios** | Más de 3000 personas en 128 países |
| **Mantenedor** | Dibbayajyoti Roy |
| **Código fuente** | <https://github.com/DibbayajyotiRoy/fresco> |
| **Sitio web** | <https://fresco.dibbayajyoti.com> |

## Colaborar y comentarios

Los informes de errores, las solicitudes de funciones y los PR son bienvenidos: abre un [issue](https://github.com/DibbayajyotiRoy/fresco/issues) o usa el cuadro de diálogo de comentarios de la aplicación. Consulta [CONTRIBUTING.md](CONTRIBUTING.md).

## Licencia

[GPL-3.0-or-later](LICENSE): libre y de código abierto.

---

<sub>Fresco es una aplicación gratuita y de código abierto de fondos de pantalla en vivo y de video para Linux (X11 y Wayland), con widgets de letras, reloj, visualizador de audio y portada del álbum dibujados en el fondo de pantalla. Es una alternativa nativa a Wallpaper Engine para Linux y un complemento de música y reloj para herramientas como Conky. Lo usan más de 3000 personas en 128 países. Última actualización: 2026-10-16.</sub>
