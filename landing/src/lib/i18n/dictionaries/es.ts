import type { Dictionary } from "./en";

/**
 * Español (neutro). Los nombres de producto y los términos de plataforma se
 * mantienen en inglés (Fresco, X11, Wayland, layer-shell, mpv, GPL-3.0 ...),
 * como es habitual en la documentación de Linux en español.
 */
export const es: Dictionary = {
  meta: {
    title:
      "Fresco - Fondo de pantalla animado para Linux | Alternativa gratuita a Wallpaper Engine",
    description:
      "Aplicación gratuita y de código abierto de fondos de pantalla animados para Linux. Catálogo integrado, vídeos o GIF en el escritorio, fondo por monitor y cambio automático entre día y noche. Aceleración por hardware en X11 y Wayland.",
    ogTitle: "Fresco - Fondos de pantalla animados para Linux",
    ogDescription:
      "Catálogo integrado, fondo por monitor, horarios de día y noche y reproducción acelerada por hardware con un uso de CPU casi nulo en X11 y Wayland. Una alternativa gratuita a Wallpaper Engine.",
    twitterDescription:
      "Fondos de pantalla animados para Linux con aceleración por hardware, en X11 y Wayland. Una alternativa gratuita y de código abierto a Wallpaper Engine.",
    ogImageAlt:
      "Fresco. Por fin, un fondo de pantalla para Linux que simplemente funciona.",
    keywords: [
      "fondo de pantalla animado linux",
      "fondo de pantalla en video linux",
      "wallpaper animado ubuntu",
      "alternativa wallpaper engine linux",
    ],
  },

  nav: {
    home: "Inicio de Fresco",
    features: "Funciones",
    compare: "Comparar",
    whatsNew: "Novedades",
    download: "Descargar",
    cta: "Obtener Fresco",
    star: "Dar una estrella a Fresco en GitHub",
    starWithCount: (n: string) =>
      `Dar una estrella a Fresco en GitHub (${n} estrellas)`,
  },

  language: {
    label: "Idioma",
    change: "Cambiar idioma",
  },

  theme: {
    toggle: "Cambiar tema",
    light: "Claro",
    dark: "Oscuro",
  },

  hero: {
    titleLead: "Por fin, un fondo de pantalla para Linux",
    /** Separator before the accented tail; empty where CJK needs none. */
    titleGap: " ",
    titleEm: "que simplemente funciona.",
    body: "Pon cualquier vídeo, GIF o imagen en tu escritorio Linux. La reproducción acelerada por hardware mantiene la CPU casi a cero, en X11 y en Wayland. Cierra la aplicación: el daemon sigue reproduciendo.",
    install: "Instalar Fresco",
    star: "Dar estrella en GitHub",
  },

  stats: {
    ariaLabel: "Estadísticas del proyecto",
    downloads: "descargas totales",
    downloadsUnknown: "descargas en github",
    stars: "estrellas en github",
    version: "última versión",
    license: "gratuito y de código abierto",
  },

  glance: {
    ariaLabel: "Fresco de un vistazo",
    caption: "fresco de un vistazo",
    labelWhat: "qué es",
    labelPlatforms: "plataformas",
    labelWidgets: "widgets",
    labelLicense: "licencia",
    labelInstall: "instalación",
    what: "Fresco es una aplicación gratuita y de código abierto de fondos de pantalla animados para Linux: pone vídeos, GIF, imágenes, presentaciones y listas de reproducción como fondo de escritorio animado, con decodificación por hardware en la GPU. Una alternativa gratuita a Wallpaper Engine y una interfaz gráfica para mpvpaper en Wayland.",
    platforms:
      "Cualquier escritorio X11 (Ubuntu, Pop!_OS, Linux Mint, Debian), además de los compositores Wayland con layer-shell: COSMIC, Hyprland, Sway y KDE Plasma 6. En GNOME con Wayland recurre a un fotograma estático.",
    widgets:
      "Cuatro widgets dibujados en el propio fondo de pantalla, no en una ventana: letras de canciones sincronizadas, un reloj con seis temas, un visualizador de audio y la portada del álbum en un disco que gira. Nada flota sobre tus ventanas y nada intercepta un clic. Todos vienen desactivados. No están disponibles en GNOME con Wayland, que no tiene superficie de fondo animado.",
    licenseLead: "GPL-3.0, gratis para siempre.",
    licenseLink: "Código en GitHub",
    licenseTail: "Hecho con Rust, GTK4 y mpv.",
  },

  features: {
    kicker: "funciones",
    title: "Cualquier medio. Cualquier monitor. Sin dramas de CPU.",
    lead: "Fresco pone fondos de pantalla en vídeo, GIF, imagen, presentación y lista de reproducción en X11 y Wayland, decodificados en la GPU, de modo que un fondo animado cuesta casi lo mismo que uno estático. La ficha técnica completa:",
    manifest: (n: number) => `manifiesto: ${n} funciones`,
    /** Sentence-final mark after each row title. */
    titleSuffix: ".",
    thCapability: "Función",
    thWhatYouGet: "Qué obtienes",
    thStatus: "Estado",
    footnote:
      "gnome wayland: recurre a un fotograma estático (mutter no expone superficie animada), y los widgets también necesitan esa superficie, así que allí no están disponibles. todo lo demás de arriba funciona.",
    tally: (shipping: number, total: number, soon: number) =>
      `${shipping} de ${total} disponibles · ${soon} en vista previa · 0 descontinuadas`,
    rows: {
      hwDecode: {
        tag: "decodif. hw",
        title: "Reproducción acelerada por hardware",
        description:
          "La decodificación se ejecuta en la GPU a través de mpv (VA-API o NVDEC). Un fondo de pantalla en 4K cuesta casi la misma CPU que una imagen estática.",
        status: "cpu casi nula",
      },
      sessions: {
        tag: "sesiones",
        title: "X11 y Wayland",
        description:
          "Un backend de ventana de escritorio en cualquier sesión X11, más un backend layer-shell para COSMIC, Hyprland, Sway y KDE Plasma 6. GNOME con Wayland recibe un fotograma estático.",
        status: "x11 · layer-shell",
      },
      catalog: {
        tag: "catálogo",
        title: "Catálogo de fondos integrado",
        description:
          "Explora fondos seleccionados y con licencia dentro de la aplicación (menú y luego Explorar fondos) y pon uno en dos clics. También puedes pegar un enlace directo.",
        status: "en la app",
      },
      video: {
        tag: "vídeo · gif",
        title: "Fondos en vídeo y GIF",
        description:
          "Reproduce en bucle cualquier mp4, webm, mkv o GIF animado en tu escritorio.",
        status: "mp4 webm mkv gif",
      },
      slideshow: {
        tag: "presentación",
        title: "Presentaciones con transiciones",
        description:
          "Rota una carpeta de imágenes con fundido cruzado, fundido o Ken Burns.",
        status: "4 transiciones",
      },
      playlist: {
        tag: "lista",
        title: "Listas de reproducción de vídeo",
        description:
          "Encola varios clips y deja que Fresco los vaya alternando.",
        status: "ciclo automático",
      },
      lyrics: {
        tag: "letras · reloj",
        title: "Widgets de letra y reloj",
        description:
          "Letras sincronizadas con lo que suene a través de MPRIS (primero el .lrc local, luego LRCLIB) y un reloj en uno de seis temas. Se dibujan en el fondo, así que nada flota sobre tus ventanas. Desactivados por defecto.",
        status: "desactivado por defecto",
      },
      visualiser: {
        tag: "visualizador",
        title: "Visualizador de audio y portada",
        description:
          "Cinco estilos (Bars, Mirror, Wave, Dots, Ring) con selector de color, mezcla de dos colores o arcoíris, más la portada de la pista actual en un disco que gira. El visualizador pide permiso antes de escuchar tu audio.",
        status: "0,8% de un núcleo",
      },
      editor: {
        tag: "editor",
        title: "Recortar y rotar",
        description:
          "Arrastra un marco para elegir la región y rota 90 grados para enderezar clips tumbados. Ambos siguen siendo zero-copy en la GPU.",
        status: "zero-copy",
      },
      audio: {
        tag: "audio",
        title: "Sonido por fondo de pantalla",
        description:
          "Activa el sonido de un vídeo y ajusta su volumen. Fresco recuerda la elección para ese fondo.",
        status: "por fondo",
      },
      displays: {
        tag: "pantallas",
        title: "Fondo por pantalla",
        description:
          "Haz clic derecho en cualquier fondo y elige Poner en una pantalla concreta. Cada monitor puede tener el suyo.",
        status: "por monitor",
      },
      schedule: {
        tag: "horario",
        title: "Horarios de día y noche",
        description:
          "Dos fondos, dos horas de cambio, intercambiados automáticamente por el daemon. Franjas horarias y cambio solar desde la configuración.",
        status: "automático",
      },
      power: {
        tag: "energía",
        title: "Consciente de la energía",
        description:
          "Pausa con batería y pausa automáticamente por monitor cuando una ventana allí pasa a pantalla completa.",
        status: "pausa automática",
      },
      newTab: {
        tag: "pestaña nueva",
        title: "Tu fondo en cada pestaña nueva",
        description:
          "Una extensión de navegador (Chrome, Brave, Edge, Firefox) refleja el fondo de tu escritorio, o una elección propia del navegador, en la página de pestaña nueva mediante un puente local que solo habla con 127.0.0.1. Ya está en el repositorio; la publicación en las tiendas está pendiente.",
        status: "próximamente",
      },
      themes: {
        tag: "temas",
        title: "Temas y acentos",
        description:
          "Claro, oscuro o según el sistema, con seis paletas de acento.",
        status: "6 paletas",
      },
    },
  },

  compare: {
    kicker: "comparar",
    title: "Fresco frente al resto de fondos animados en Linux.",
    lead: "Fresco es la única aplicación de fondos animados para Linux de esta tabla que combina interfaz gráfica, decodificación por hardware, soporte de X11 y Wayland y un catálogo integrado, gratis y con mantenimiento activo. Aquí está la comparación completa con Hidamari, Komorebi, mpvpaper y Wallpaper Engine.",
    meter: (tools: number, caps: number) =>
      `comparación · ${tools} herramientas · ${caps} funciones`,
    thFeature: "Función",
    yes: "Sí",
    no: "No",
    note: "Wallpaper Engine es un producto de pago pensado primero para Windows. Komorebi ya no recibe mantenimiento.",
    detailLabel: "Comparar en detalle:",
    vs: (tool: string) => `Fresco vs ${tool}`,
    rows: {
      gui: "Aplicación gráfica, sin terminal",
      x11: "Funciona en X11",
      wayland: "Funciona en Wayland (layer-shell)",
      hwDecode: "Decodificación por hardware, CPU baja",
      cropRotate: "Recorte arrastrando y rotación",
      playlists: "Listas de reproducción",
      slideshow: "Presentación de imágenes",
      library: "Biblioteca de fondos",
      catalog: "Catálogo integrado",
      perDisplay: "Fondo por pantalla (interfaz)",
      schedules: "Horarios de día y noche",
      maintained: "Mantenimiento activo",
      foss: "Gratuito y de código abierto",
    },
    cells: {
      partial: "Parcial",
      manual: "Manual",
      compositorOff: "Sin compositor",
      cropOnly: "Solo recorte",
      workshop: "Workshop",
    },
  },

  whatsNew: {
    kicker: (version: string) => `novedades · v${version}`,
    title: "Cuatro widgets, pintados en el fondo de pantalla.",
    lead: (version: string) =>
      `Lo que llegó en la v${version}. Sin ventana extra, sin nada que clicar, idéntico en X11 y en layer-shell. Los cuatro vienen desactivados y, con música sonando y todos encendidos, el coste medido fue del 0,8% de un núcleo de CPU. Cada entrada de aquí está reproducida en el CHANGELOG de GitHub.`,
    changelog: "Changelog completo",
    patch: (n: string) => `parche ${n}`,
    items: {
      lyrics: {
        title: "Letras sincronizadas",
        body: "La línea actual, al ritmo de lo que suene a través de MPRIS. Primero archivos .lrc locales, luego LRCLIB. Cuatro ajustes predefinidos y un desfase de sincronía.",
      },
      clock: {
        title: "Reloj, seis temas",
        body: "Digital, Minimal, Segment, Stacked, Wordy y Card, un panel translúcido con una esfera analógica dibujada. 12 o 24 horas, fecha opcional.",
      },
      visualizer: {
        title: "Visualizador de audio",
        body: "Bars, Mirror, Wave, Dots o Ring, con selector de color, mezcla de dos colores o arcoíris. Pide permiso antes de escuchar tu audio.",
      },
      disc: {
        title: "Portada sobre un disco",
        body: "La portada de la pista actual en un disco que gira. Deja de girar en el instante en que la reproducción se pausa.",
      },
    },
  },

  howItWorks: {
    kicker: "cómo funciona",
    title: "Tres clics y a olvidarse.",
    lead: "Abre Fresco, pulsa añadir, pulsa poner, cierra. El daemon mantiene el fondo en marcha, incluso después de reiniciar.",
    step: (n: string) => `paso ${n}`,
    steps: {
      pick: {
        title: "Elige tu medio",
        description:
          "Abre Fresco desde el menú de aplicaciones y elige un vídeo, GIF, imagen, carpeta o lista de reproducción.",
      },
      set: {
        title: "Pulsa Poner",
        description:
          "Ponlo como fondo de pantalla. Empieza a reproducirse en tu escritorio al instante.",
      },
      close: {
        title: "Cierra la aplicación",
        description:
          "Cierra la ventana. Un daemon ligero mantiene el fondo en marcha, incluso tras reiniciar.",
      },
    },
  },

  videos: {
    kicker: "míralo funcionar",
    title: "Menos de un minuto cada uno. Sin narración.",
    lead: "Grabaciones de pantalla cortas de Fresco en un escritorio real. No se carga nada de YouTube hasta que pulsas reproducir.",
    more: "Más en YouTube",
    inDevelopment: "en desarrollo",
    play: (title: string) => `Reproducir: ${title}`,
    items: {
      "YWzD3-xkCEc": {
        tag: "añadir por enlace",
        blurb:
          "Copia un enlace de Pinterest, pégalo en Fresco y ponlo como fondo. Sin descargas ni malabares con archivos.",
      },
      C1MqrhGkovQ: {
        tag: "widgets de letra",
        blurb:
          "Letras sincronizadas y un reloj dibujados en un fondo animado en Wayland y X11. Llegaron en la v1.1.36, junto con un visualizador de audio y un disco con la portada.",
      },
    },
  },

  supported: {
    kicker: "entornos probados",
    title: "Dónde funciona Fresco.",
    lead: "En cualquier escritorio X11, incluido el DDE de Deepin 25, y en los compositores Wayland con layer-shell (COSMIC, Hyprland, Sway y KDE Plasma 6), en las distribuciones Debian y Ubuntu más populares. GNOME con Wayland recibe un fotograma estático.",
    deployed: (distros: number, formats: number) =>
      `probado: 6 compositores con animación · 1 alternativa estática · ${distros} distros · ${formats} formatos`,
    sessionsTitle: "sesiones y compositores",
    distrosTitle: (n: number) => `distribuciones probadas · ${n}`,
    formatsTitle: (n: number) => `formatos compatibles · ${n}`,
    live: "Fondo animado",
    fallback: "Fotograma estático",
    sessions: {
      x11: {
        label: "X11 (cualquier escritorio)",
        detail: "GNOME, KDE, XFCE, MATE, Cinnamon, Budgie",
      },
      deepin: {
        label: "Deepin 25 (DDE, X11)",
        detail:
          "Adaptación automática a DDE, los iconos siguen visibles. Verificado por la comunidad en Deepin 25 Community build1.",
      },
      wayland: {
        label: "Wayland layer-shell",
        detail: "COSMIC, Hyprland, Sway, KDE Plasma 6, wlroots",
      },
      gnome: {
        label: "GNOME en Wayland",
        detail: "Fotograma estático (Mutter no tiene superficie animada)",
      },
    },
    fieldReport: "informe de campo · deepin 25",
    verifiedEnv: "entorno verificado",
    testimonialRole: "Probador de la comunidad Deepin",
    envLabels: {
      session: "sesión",
      os: "so",
      gpu: "gpu",
    },
    footnote:
      "deepin 25 usa x11 por defecto, y es la sesión en la que fresco está verificado allí. treeland, el compositor wayland propio de deepin, sigue en desarrollo, así que fresco no afirma nada todavía sobre deepin en wayland.",
  },

  download: {
    kicker: "descargar",
    title: "Instálalo en Debian, Ubuntu, Pop!_OS y Mint.",
    badge: "x11 · wayland",
    lead: "El instalador oficial de una línea o el paquete .deb. Cualquiera de las dos vías se copia al portapapeles y se ejecuta al instante. Fresco sigue reproduciendo después de cerrar la ventana.",
    cardTitle: "instalación en una línea",
    cardBody:
      "Ejecuta esto en una terminal. Descarga e instala el .deb más reciente por ti, siempre la versión más nueva:",
    terminalTitle: "fresco install",
    aptComment: "¿ya tienes el .deb descargado?",
    releases: "Ver todas las versiones",
    gpuNote:
      "Para el menor uso de CPU, instala el controlador de decodificación por hardware de tu GPU (el controlador Intel media VA, los controladores VA de Mesa o el controlador propietario de NVIDIA para NVDEC).",
    copy: "Copiar",
    copied: "Copiado",
  },

  faq: {
    kicker: "preguntas frecuentes",
    title: "Preguntas, respondidas.",
    lead: "Todo lo que necesitas saber antes de poner tu primer fondo de pantalla animado en Linux.",
    items: [
      {
        q: "¿Existe un Wallpaper Engine para Linux?",
        a: "Sí. Fresco es una aplicación gratuita y de código abierto de fondos animados para Linux que funciona como Wallpaper Engine: elige un vídeo, GIF o imagen y ponlo como fondo de escritorio animado. Está pensada para usarse con interfaz gráfica y no necesita Steam ni Proton.",
      },
      {
        q: "¿Cómo pongo un vídeo como fondo en Ubuntu o Pop!_OS?",
        a: "Instala el .deb de Fresco, ábrelo desde el menú de aplicaciones, pulsa Añadir, elige tu vídeo, recórtalo o rótalo si quieres y pulsa Poner como fondo de pantalla. Cierra la aplicación y el vídeo sigue reproduciéndose como fondo de escritorio.",
      },
      {
        q: "¿Un fondo en vídeo consume CPU o batería?",
        a: "No. Fresco decodifica el vídeo en la GPU a través de mpv (VA-API y NVDEC), así que el uso de CPU se queda cerca de cero y la memoria ronda los 120 a 150 MB. Puede pausarse automáticamente con batería y se pausa solo en cualquier monitor que tenga una ventana a pantalla completa.",
      },
      {
        q: "¿Fresco funciona en Wayland y en el escritorio COSMIC?",
        a: "Sí. Fresco ejecuta fondos animados en compositores Wayland con layer-shell mediante un backend mpvpaper incluido y supervisado: COSMIC (Pop!_OS 24.04), Hyprland, Sway, KDE Plasma 6 y otros compositores wlroots. Desde la v1.1.1 incluye dos compilaciones de mpvpaper y comprueba cuál usar en tiempo de ejecución, así que funciona tanto en distribuciones con libmpv1 como con libmpv2. En X11 funciona en cualquier escritorio.",
      },
      {
        q: "¿Fresco funciona en GNOME?",
        a: "En GNOME con sesión X11, sí, con fondos animados completos. En GNOME con Wayland, Mutter no expone una superficie de fondo animado, así que Fresco muestra un fotograma estático del fondo elegido en lugar de fingir que anima.",
      },
      {
        q: "¿Un fondo en vídeo puede reproducir sonido?",
        a: "Sí. Cada fondo recuerda su propio estado de silencio y su volumen, así que puedes activar el sonido de un vídeo concreto y la elección se mantiene cada vez que lo pones. Los fondos empiezan silenciados por defecto.",
      },
      {
        q: "¿Puedo recortar o rotar un fondo de pantalla?",
        a: "Sí. El editor tiene un marco de recorte que se arrastra y una rotación de 90 grados, así que puedes elegir la región exacta o enderezar un vídeo de móvil grabado de lado. Ambos se aplican en la GPU y se recuerdan por fondo.",
      },
      {
        q: "¿El fondo se mantiene después de reiniciar?",
        a: "Sí. Fresco añade una entrada de inicio automático que restaura tu fondo animado al iniciar sesión, y la repara sola si falta. Puedes desactivarlo en los ajustes.",
      },
      {
        q: "¿Qué formatos de medios son compatibles?",
        a: "Vídeo en bucle (mp4, webm, mkv, avi, mov), GIF animados, imágenes estáticas (jpg, png, webp), una carpeta de imágenes como presentación con transiciones de fundido cruzado, fundido, deslizamiento o Ken Burns, y listas de reproducción con varios vídeos.",
      },
      {
        q: "¿Admite varios monitores?",
        a: "Sí. Puedes poner un fondo distinto en cada pantalla, y Fresco pausa el fondo de esa salida cuando una ventana allí pasa a pantalla completa. La conexión en caliente de monitores es inmediata en X11; en Wayland una pantalla recién conectada se detecta en la siguiente aplicación (la detección automática llegará con el motor v1.0).",
      },
      {
        q: "¿En qué se diferencia Fresco de Wallpaper Engine?",
        a: "Wallpaper Engine es un producto de pago pensado primero para Windows que en Linux solo funciona mediante Steam Play y Proton. Fresco es gratuito, de código abierto (GPL-3.0) y nativo de Linux: sin Steam, sin Proton, sin capa de compatibilidad. En lugar del Steam Workshop tiene un catálogo integrado de fondos seleccionados y con licencia, y admite X11 y compositores Wayland con layer-shell directamente.",
      },
      {
        q: "¿En qué se diferencia Fresco de Hidamari, Komorebi y mpvpaper?",
        a: "Fresco está pensado para la interfaz gráfica, está acelerado por hardware y gestiona fondos en vídeo, GIF, imagen, presentación y lista de reproducción en una sola aplicación, tanto en X11 como en Wayland. Tiene mantenimiento activo, a diferencia de Komorebi, y no necesita línea de comandos, a diferencia de mpvpaper.",
      },
      {
        q: "¿Dónde encuentro fondos de pantalla animados para Linux?",
        a: "Dentro del propio Fresco. El catálogo integrado (menú y luego Explorar fondos) ofrece fondos en vídeo seleccionados y con la licencia en regla que puedes poner en dos clics, con la licencia y el autor visibles en cada elemento. También puedes pegar la URL directa de un vídeo o una imagen, o añadir tus propios archivos.",
      },
      {
        q: "¿Puede cambiar el fondo automáticamente entre día y noche?",
        a: "Sí. Abre el menú, elige Avanzado y luego Fondo de día y noche: elige dos fondos y las horas de cambio, y el daemon los intercambia automáticamente sin reiniciar. Las franjas horarias arbitrarias y el cambio al amanecer o al atardecer (con coordenadas manuales) están disponibles a través de config.toml.",
      },
      {
        q: "¿Cómo pongo un fondo distinto en cada monitor?",
        a: "Haz clic derecho en cualquier fondo de la biblioteca y elige Poner en una pantalla concreta. Cada monitor conectado aparece con su resolución. Elegir Mostrar el predeterminado en todas las pantallas borra las excepciones por monitor.",
      },
      {
        q: "¿Puedo mostrar la letra de las canciones en mi escritorio Linux?",
        a: "Sí. Fresco dibuja letras sincronizadas sobre tu fondo, siguiendo lo que esté sonando en tu sistema a través de MPRIS: navegadores, aplicaciones de música, reproductores de vídeo. Hay cuatro ajustes predefinidos, una cuadrícula de nueve posiciones, un control de desfase de sincronía, una línea siguiente opcional y título y artista opcionales. Las letras vienen primero de un archivo .lrc local y luego de LRCLIB, una base de datos gratuita gestionada por la comunidad. Firefox es el reproductor más fiable; el cliente nativo de Spotify para Linux informa mal la posición de reproducción, aunque Spotify en el navegador va bien.",
      },
      {
        q: "¿Linux tiene widgets de escritorio como Conky?",
        a: "Sí, y Fresco añade cuatro que no necesitan panel, ni extensión, ni soporte de tu escritorio: letras sincronizadas, un reloj con seis temas, un visualizador de audio con cinco estilos y la portada de la pista actual en un disco que gira. Se pintan en el propio fondo en vez de en una ventana, así que nunca quedan por encima de tus ventanas, nunca interceptan un clic y funcionan en escritorios que no tienen capa de widgets propia, incluidos COSMIC, Hyprland y Sway. Los cuatro vienen desactivados. A diferencia de Conky todavía no hay widgets de monitorización del sistema, así que no hay lecturas de CPU, RAM o red. GNOME con Wayland es el único sitio donde los widgets no pueden funcionar, porque no hay superficie de fondo animado sobre la que dibujar.",
      },
      {
        q: "¿Puedo tener un visualizador de música en el fondo de escritorio?",
        a: "Sí. El visualizador de audio de Fresco reacciona a lo que esté reproduciendo tu sistema, en uno de cinco estilos (Bars, Mirror, Wave, Dots, Ring) con selector de color, mezcla de dos colores o arcoíris. Viene desactivado y pide consentimiento la primera vez que lo activas, porque tiene que escuchar tu salida de audio. Con música sonando y los cuatro widgets encendidos, el coste medido fue del 0,8% de un núcleo de CPU, casi todo por la captura de audio, porque nada se redibuja salvo que su contenido cambie.",
      },
      {
        q: "¿Fresco es gratis?",
        a: "Sí. Fresco es completamente gratuito y de código abierto bajo la licencia GPL-3.0. No hay versión de pago.",
      },
    ],
  },

  footer: {
    github: "GitHub",
    license: "Licencia",
    tagline: "rust + gtk4 + mpv",
    sound: "Activar o silenciar el sonido",
  },

  featureList: [
    "Catálogo integrado de fondos seleccionados y con licencia",
    "Fondos en vídeo, GIF, imagen, presentación y lista de reproducción",
    "Widgets dibujados en el fondo: letras sincronizadas, reloj, visualizador de audio, portada del álbum",
    "Añadir fondos desde una URL directa",
    "Horarios de fondo de día y noche (más franjas horarias y modo solar desde la configuración)",
    "Fondo por pantalla desde la interfaz gráfica",
    "Recuperación automática del audio cuando el servidor de sonido arranca tarde",
    "Socket de control JSON programable",
    "Reproducción acelerada por hardware (VA-API, NVDEC)",
    "Funciona en X11 y en compositores Wayland con layer-shell",
    "Editor de recorte arrastrando y rotación de 90 grados",
    "Sonido y volumen por fondo de pantalla",
    "Transiciones de presentación (fundido cruzado, fundido, deslizamiento, Ken Burns)",
    "Biblioteca de fondos con búsqueda",
    "Un fondo distinto en cada monitor",
    "Pausa con batería y pausa automática a pantalla completa",
    "Se restaura automáticamente al iniciar sesión",
    "Temas y colores de acento",
  ],

  softwareDescription:
    "Fresco es una aplicación gratuita y de código abierto de fondos de pantalla animados para Linux. Pone fondos en vídeo, GIF, imagen, presentación y lista de reproducción como fondo de escritorio animado, con reproducción acelerada por hardware, y puede dibujar cuatro widgets en el propio fondo: letras sincronizadas, un reloj, un visualizador de audio y la portada del álbum en un disco que gira. Una alternativa gratuita a Wallpaper Engine para Pop!_OS, Ubuntu, Linux Mint, Debian y elementary OS, en X11 y en compositores Wayland con layer-shell (COSMIC, Hyprland, Sway, KDE Plasma 6).",
};
