import type { Dictionary } from "./en";

/**
 * Deutsch. Produkt- und Plattformbegriffe bleiben englisch (Fresco, X11,
 * Wayland, layer-shell, mpv, GPL-3.0 ...), wie in deutscher Linux-Doku
 * üblich. Durchgehend "du", passend zum Ton der Originalseite.
 */
export const de: Dictionary = {
  meta: {
    title:
      "Fresco - Live-Hintergrund für Linux | Kostenlose Wallpaper-Engine-Alternative",
    description:
      "Kostenlose Open-Source-App für animierte Hintergründe unter Linux. Eingebauter Katalog, Videos oder GIFs als Desktop, Hintergrund pro Monitor, Tag- und Nachtwechsel. Hardwarebeschleunigt unter X11 und Wayland.",
    ogTitle: "Fresco - Live-Hintergründe für Linux",
    ogDescription:
      "Eingebauter Hintergrundkatalog, Hintergrund pro Monitor, Tag- und Nachtzeitpläne und hardwarebeschleunigte Wiedergabe mit nahezu null CPU unter X11 und Wayland. Eine kostenlose Wallpaper-Engine-Alternative.",
    twitterDescription:
      "Hardwarebeschleunigte Live-Hintergründe für Linux, unter X11 und Wayland. Eine kostenlose Open-Source-Alternative zu Wallpaper Engine.",
    ogImageAlt:
      "Fresco. Endlich ein Linux-Hintergrund, der einfach funktioniert.",
    keywords: [
      "live hintergrund linux",
      "video hintergrund linux",
      "animierter hintergrund ubuntu",
      "wallpaper engine linux alternative",
    ],
  },

  nav: {
    home: "Fresco Startseite",
    features: "Funktionen",
    compare: "Vergleich",
    whatsNew: "Neu",
    download: "Download",
    cta: "Fresco holen",
    star: "Fresco auf GitHub einen Stern geben",
    starWithCount: (n: string) =>
      `Fresco auf GitHub einen Stern geben (${n} Sterne)`,
    menu: "Menü",
  },

  language: {
    label: "Sprache",
    change: "Sprache wechseln",
  },

  theme: {
    toggle: "Design wechseln",
    light: "Hell",
    dark: "Dunkel",
  },

  hero: {
    titleLead: "Endlich ein Linux-Hintergrund,",
    /** Separator before the accented tail; empty where CJK needs none. */
    titleGap: " ",
    titleEm: "der einfach funktioniert.",
    body: "Mach jedes Video, GIF oder Bild zu deinem Linux-Desktop. Hardwarebeschleunigte Wiedergabe hält die CPU nahe null, unter X11 und Wayland. Schließ die App: Der Daemon spielt weiter.",
    install: "Fresco installieren",
    star: "Stern auf GitHub",
    newUsers24h: (n: string) =>
      `${n} Menschen haben in den letzten 24 Stunden mit Fresco angefangen`,
    builtBy: "Entwickelt von",
    activeToday: (n: string) =>
      `${n} Menschen haben Fresco in den letzten 24 Stunden genutzt`,
    newToday: (n: string) => `${n} neu`,
  },

  stats: {
    ariaLabel: "Projektstatistik",
    downloads: "Downloads gesamt",
    downloadsUnknown: "Downloads auf GitHub",
    stars: "GitHub-Sterne",
    version: "Neueste Version",
    license: "kostenlos und quelloffen",
    users: "Nutzer",
    countries: "Länder",
    cohortNote:
      "Nutzer und Länder werden aus Installationen gezählt, die der anonymen Telemetrie zugestimmt haben.",
    active24h: "aktiv in den letzten 24 h",
  },

  testimonials: {
    kicker: "Stimmen",
    title: (users: string, countries: string) =>
      `Läuft auf ${users} Linux-Desktops in ${countries} Ländern.`,
    lead: "Unbearbeitete Rückmeldungen von Menschen, die Fresco nutzen, direkt aus der App gesendet. Anonymes Feedback wird nur mit dem Land angegeben.",
    fromCountry: (country: string) => `Fresco-Nutzer · ${country}`,
    anonymous: "Fresco-Nutzer",
    namedLabel: "Erfahrungsbericht",
    pause: "Stimmen pausieren",
    play: "Stimmen abspielen",
  },

  glance: {
    ariaLabel: "Fresco auf einen Blick",
    caption: "Fresco auf einen Blick",
    labelWhat: "Was es ist",
    labelPlatforms: "Plattformen",
    labelWidgets: "Widgets",
    labelLicense: "Lizenz",
    labelInstall: "Installation",
    what: "Fresco ist eine kostenlose Open-Source-App für animierte Hintergründe unter Linux: Sie setzt Videos, GIFs, Bilder, Diashows und Playlists als bewegten Desktophintergrund, mit Hardware-Dekodierung auf der GPU. Eine kostenlose Wallpaper-Engine-Alternative und unter Wayland zugleich eine Oberfläche für mpvpaper.",
    platforms:
      "Jeder X11-Desktop (Ubuntu, Pop!_OS, Linux Mint, Debian) sowie Wayland-Compositoren mit layer-shell: COSMIC, Hyprland, Sway, KDE Plasma 6. Unter GNOME mit Wayland wird auf ein Standbild zurückgegriffen.",
    widgets:
      "Vier Widgets, die in den Hintergrund selbst gezeichnet werden statt in ein Fenster: zeitsynchroner Songtext, eine Uhr mit sechs Designs, ein Audio-Visualizer und das Albumcover auf einer drehenden Schallplatte. Nichts schwebt über deinen Fenstern, nichts fängt einen Klick ab. Alle sind standardmäßig aus. Unter GNOME mit Wayland nicht verfügbar, da es dort keine Fläche für animierte Hintergründe gibt.",
    licenseLead: "GPL-3.0, für immer kostenlos.",
    licenseLink: "Quellcode auf GitHub",
    licenseTail: "Gebaut mit Rust, GTK4 und mpv.",
  },

  features: {
    kicker: "Funktionen",
    title: "Jedes Medium. Jeder Monitor. Kein CPU-Drama.",
    lead: "Fresco setzt Video-, GIF-, Bild-, Diashow- und Playlist-Hintergründe unter X11 und Wayland, dekodiert auf der GPU, sodass ein bewegter Hintergrund etwa so viel kostet wie ein statischer.",
    manifest: (n: number) => `Manifest: ${n} Funktionen`,
    /** Sentence-final mark after each row title. */
    titleSuffix: ".",
    thCapability: "Funktion",
    thWhatYouGet: "Was du bekommst",
    thStatus: "Status",
    footnote:
      "GNOME Wayland: Standbild als Rückfalllösung (Mutter stellt keine animierte Fläche bereit), und die Widgets brauchen dieselbe Fläche, sind dort also nicht verfügbar. Alles andere oben läuft.",
    tally: (shipping: number, total: number, soon: number) =>
      `${shipping} von ${total} veröffentlicht · ${soon} in der Vorschau · 0 eingestellt`,
    rows: {
      hwDecode: {
        tag: "HW-Dekodierung",
        title: "Hardwarebeschleunigte Wiedergabe",
        description:
          "Die Dekodierung läuft über mpv auf der GPU (VA-API oder NVDEC). Ein 4K-Videohintergrund kostet etwa so viel CPU wie ein Standbild.",
        status: "Nahezu null CPU",
      },
      sessions: {
        tag: "Sitzungen",
        title: "X11 und Wayland",
        description:
          "Ein Desktopfenster-Backend auf jedem X11-Desktop, dazu ein layer-shell-Backend für COSMIC, Hyprland, Sway und KDE Plasma 6. GNOME mit Wayland bekommt ein Standbild.",
        status: "X11 · layer-shell",
      },
      catalog: {
        tag: "Katalog",
        title: "Eingebauter Hintergrundkatalog",
        description:
          "Kuratierte, lizenzierte Hintergründe direkt in der App durchsuchen (Menü, dann Hintergründe durchsuchen) und mit zwei Klicks setzen. Ein direkter Link lässt sich ebenfalls einfügen.",
        status: "In der App",
      },
      video: {
        tag: "Video · GIF",
        title: "Video- und GIF-Hintergründe",
        description:
          "Jedes mp4, webm, mkv oder animierte GIF als Desktop in Schleife abspielen.",
        status: "mp4 webm mkv gif",
      },
      slideshow: {
        tag: "Diashow",
        title: "Diashows mit Übergängen",
        description:
          "Einen Bildordner mit Überblendung, Ausblenden oder Ken Burns durchwechseln.",
        status: "4 Übergänge",
      },
      playlist: {
        tag: "Playlist",
        title: "Video-Playlists",
        description:
          "Mehrere Clips einreihen und Fresco der Reihe nach abspielen lassen.",
        status: "Auto-Wechsel",
      },
      lyrics: {
        tag: "Songtext · Uhr",
        title: "Songtext- und Uhr-Widget",
        description:
          "Zeitsynchroner Songtext zu allem, was über MPRIS läuft (zuerst lokale .lrc, dann LRCLIB), und eine Uhr in einem von sechs Designs. In den Hintergrund gezeichnet, also schwebt nichts über deinen Fenstern. Standardmäßig aus.",
        status: "Standardmäßig aus",
      },
      visualiser: {
        tag: "Visualizer",
        title: "Audio-Visualizer und Albumcover",
        description:
          "Fünf Stile (Bars, Mirror, Wave, Dots, Ring) mit Farbwähler, Zweifarbverlauf oder Regenbogen, dazu das Cover des laufenden Titels auf einer drehenden Schallplatte. Der Visualizer fragt, bevor er dein Audio mithört.",
        status: "0,8 % eines Kerns",
      },
      editor: {
        tag: "Editor",
        title: "Zuschneiden und drehen",
        description:
          "Zieh einen Rahmen für den Ausschnitt, dreh um 90 Grad, um querliegende Clips zu richten. Beides bleibt zero-copy auf der GPU.",
        status: "Zero-Copy",
      },
      audio: {
        tag: "Audio",
        title: "Ton pro Hintergrund",
        description:
          "Ein Video lauter stellen und die Lautstärke festlegen. Fresco merkt sich die Wahl für diesen Hintergrund.",
        status: "Pro Hintergrund",
      },
      displays: {
        tag: "Bildschirme",
        title: "Hintergrund pro Bildschirm",
        description:
          "Rechtsklick auf einen Hintergrund, dann Auf einem bestimmten Bildschirm setzen. Jeder Monitor kann seinen eigenen haben.",
        status: "Pro Monitor",
      },
      schedule: {
        tag: "Zeitplan",
        title: "Tag- und Nachtzeitpläne",
        description:
          "Zwei Hintergründe, zwei Wechselzeiten, automatisch vom Daemon getauscht. Zeitfenster und Sonnenstandswechsel über die Konfiguration.",
        status: "Automatisch",
      },
      power: {
        tag: "Energie",
        title: "Energiebewusst",
        description:
          "Pausiert im Akkubetrieb und pausiert pro Monitor automatisch, sobald dort ein Fenster in den Vollbildmodus geht.",
        status: "Auto-Pause",
      },
      newTab: {
        tag: "Neuer Tab",
        title: "Dein Hintergrund in jedem neuen Tab",
        description:
          "Eine begleitende Browsererweiterung (Chrome, Brave, Edge, Firefox) spiegelt deinen Desktophintergrund oder eine eigene Browserauswahl auf die Neuer-Tab-Seite, über eine lokale Brücke, die nur mit 127.0.0.1 spricht. Heute im Repository; die Store-Veröffentlichung steht aus.",
        status: "Bald verfügbar",
      },
      themes: {
        tag: "Designs",
        title: "Designs und Akzente",
        description:
          "Hell, dunkel oder dem System folgend, mit sechs Akzentpaletten.",
        status: "6 Paletten",
      },
    },
  },

  compare: {
    kicker: "Vergleich",
    title: "Fresco gegen das Linux-Hintergrundfeld.",
    lead: "Fresco ist die einzige aktiv gepflegte Linux-App für animierte Hintergründe in dieser Tabelle, die grafische Oberfläche, Hardware-Dekodierung, X11- und Wayland-Unterstützung und einen eingebauten Katalog vereint, und das kostenlos. Hier der vollständige Vergleich mit Hidamari, Komorebi, mpvpaper und Wallpaper Engine.",
    meter: (tools: number, caps: number) =>
      `Vergleich · ${tools} Programme · ${caps} Funktionen`,
    thFeature: "Funktion",
    yes: "Ja",
    no: "Nein",
    note: "Wallpaper Engine ist ein kostenpflichtiges, primär für Windows entwickeltes Produkt. Komorebi wird nicht mehr gepflegt.",
    detailLabel: "Im Detail vergleichen:",
    vs: (tool: string) => `Fresco vs ${tool}`,
    rows: {
      gui: "Grafische App, kein Terminal",
      x11: "Läuft unter X11",
      wayland: "Läuft unter Wayland (layer-shell)",
      hwDecode: "Hardware-Dekodierung, wenig CPU",
      cropRotate: "Zuschneiden per Ziehen und Drehen",
      playlists: "Playlists",
      slideshow: "Bilddiashow",
      library: "Hintergrundbibliothek",
      catalog: "Eingebauter Hintergrundkatalog",
      perDisplay: "Hintergrund pro Bildschirm (Oberfläche)",
      schedules: "Tag- und Nachtzeitpläne",
      maintained: "Aktiv gepflegt",
      foss: "Kostenlos und quelloffen",
    },
    cells: {
      partial: "Teilweise",
      manual: "Manuell",
      compositorOff: "Ohne Compositor",
      cropOnly: "Nur Zuschneiden",
      workshop: "Workshop",
    },
  },

  whatsNew: {
    kicker: (version: string) => `Neu · v${version}`,
    title: "Vier Desktop-Widgets, in den Hintergrund gezeichnet.",
    lead: (version: string) =>
      `Was in v${version} erschienen ist. Kein zusätzliches Fenster, nichts zum Durchklicken, identisch unter X11 und layer-shell. Alle vier sind standardmäßig aus, und mit laufender Musik und allen aktiviert lag der gemessene Aufwand bei 0,8 % eines CPU-Kerns. Jeder Eintrag hier steht auch im CHANGELOG auf GitHub.`,
    changelog: "Vollständiges Changelog",
    patch: (n: string) => `Patch ${n}`,
    items: {
      lyrics: {
        title: "Synchroner Songtext",
        body: "Die aktuelle Zeile, im Takt zu allem, was über MPRIS läuft. Zuerst lokale .lrc-Dateien, dann LRCLIB. Vier Voreinstellungen und ein Sync-Versatz.",
      },
      clock: {
        title: "Uhr, sechs Designs",
        body: "Digital, Minimal, Segment, Stacked, Wordy und Card, eine transluzente Fläche mit gezeichnetem Analogzifferblatt. 12 oder 24 Stunden, Datum optional.",
      },
      visualizer: {
        title: "Audio-Visualizer",
        body: "Bars, Mirror, Wave, Dots oder Ring, mit Farbwähler, Zweifarbverlauf oder Regenbogen. Fragt, bevor er dein Audio mithört.",
      },
      disc: {
        title: "Albumcover auf der Platte",
        body: "Das Cover des laufenden Titels auf einer drehenden Scheibe. Sie hält an, sobald die Wiedergabe pausiert.",
      },
    },
  },

  howItWorks: {
    kicker: "So funktioniert es",
    title: "Drei Klicks, dann vergiss es einfach.",
    lead: "Fresco öffnen, hinzufügen klicken, setzen klicken, schließen. Der Daemon hält den Hintergrund am Laufen, auch nach einem Neustart.",
    step: (n: string) => `Schritt ${n}`,
    steps: {
      pick: {
        title: "Medium auswählen",
        description:
          "Öffne Fresco aus dem Anwendungsmenü und wähle ein Video, GIF, Bild, einen Ordner oder eine Playlist.",
      },
      set: {
        title: "Auf Setzen klicken",
        description:
          "Setz es als Hintergrund. Es läuft sofort auf deinem Desktop.",
      },
      close: {
        title: "App schließen",
        description:
          "Schließ das Fenster. Ein schlanker Daemon hält den Hintergrund am Laufen, auch nach einem Neustart.",
      },
    },
  },

  videos: {
    kicker: "In Aktion",
    title: "Jeweils unter einer Minute. Kein Kommentar nötig.",
    lead: "Kurze Bildschirmaufnahmen von Fresco auf einem echten Desktop. Von YouTube wird nichts geladen, bis du auf Play drückst.",
    more: "Mehr auf YouTube",
    inDevelopment: "In Entwicklung",
    play: (title: string) => `Abspielen: ${title}`,
    items: {
      "YWzD3-xkCEc": {
        tag: "Per Link hinzufügen",
        blurb:
          "Pinterest-Link kopieren, in Fresco einfügen, als Hintergrund setzen. Kein Download, kein Hantieren mit Dateien.",
      },
      C1MqrhGkovQ: {
        tag: "Songtext-Widgets",
        blurb:
          "Synchroner Songtext und eine Uhr, gezeichnet in einen bewegten Hintergrund unter Wayland und X11. Erschienen in v1.1.36, zusammen mit einem Audio-Visualizer und einer Albumcover-Scheibe.",
      },
    },
  },

  supported: {
    kicker: "Getestete Umgebungen",
    title: "Wo Fresco läuft.",
    lead: "Auf jedem X11-Desktop, einschließlich DDE von deepin 25, und auf Wayland-Compositoren mit layer-shell (COSMIC, Hyprland, Sway und KDE Plasma 6), quer durch die verbreiteten Debian- und Ubuntu-Distributionen. GNOME mit Wayland bekommt ein Standbild.",
    deployed: (distros: number, formats: number) =>
      `Getestet: 6 animierte Compositoren · 1 Standbild-Rückfall · ${distros} Distributionen · ${formats} Formate`,
    sessionsTitle: "Sitzungen und Compositoren",
    distrosTitle: (n: number) => `Getestete Distributionen · ${n}`,
    formatsTitle: (n: number) => `Unterstützte Formate · ${n}`,
    live: "Animierter Hintergrund",
    fallback: "Standbild",
    sessions: {
      x11: {
        label: "X11 (jeder Desktop)",
        detail: "GNOME, KDE, XFCE, MATE, Cinnamon, Budgie",
      },
      deepin: {
        label: "Deepin 25 (DDE, X11)",
        detail:
          "Automatische DDE-Anpassung, Symbole bleiben sichtbar. Von der Community auf deepin 25 Community build1 bestätigt. Auch im deepin App Store.",
      },
      wayland: {
        label: "Wayland layer-shell",
        detail: "COSMIC, Hyprland, Sway, KDE Plasma 6, wlroots",
      },
      gnome: {
        label: "GNOME unter Wayland",
        detail: "Standbild (Mutter hat keine animierte Fläche)",
      },
    },
    fieldReport: "Praxisbericht · deepin 25",
    verifiedEnv: "Bestätigte Umgebung",
    testimonialRole: "Tester aus der deepin-Community",
    envLabels: {
      session: "Sitzung",
      os: "OS",
      gpu: "GPU",
    },
    footnote:
      "Deepin 25 nutzt standardmäßig X11, und genau in dieser Sitzung ist Fresco dort bestätigt. Treeland, deepins eigener Wayland-Compositor, ist noch in Entwicklung, deshalb macht Fresco zu deepin unter Wayland vorerst keine Aussage.",
  },

  download: {
    kicker: "Download",
    title: "Installieren unter Debian, Ubuntu, Pop!_OS und Mint.",
    badge: "X11 · Wayland",
    lead: "Der offizielle Einzeiler-Installer oder das .deb-Release. Beide Wege landen in deiner Zwischenablage und laufen sofort. Fresco spielt weiter, nachdem du das Fenster geschlossen hast.",
    cardTitle: "Installation in einer Zeile",
    cardBody:
      "Führ das in einem Terminal aus. Es lädt und installiert das neueste .deb für dich, immer die aktuellste Version:",
    terminalTitle: "fresco install",
    aptComment: "Das .deb schon heruntergeladen?",
    releases: "Alle Releases ansehen",
    gpuNote:
      "Für die geringste CPU-Last installiere den Hardware-Dekodierungstreiber deiner GPU (Intel media VA driver, Mesa VA drivers oder den proprietären NVIDIA-Treiber für NVDEC).",
    storeLabel: "Auch im deepin App Store",
    storeBody:
      "Unter deepin 25 den App Store öffnen, nach Fresco suchen und auf Installieren klicken. Updates kommen über den Store.",
    copy: "Kopieren",
    copied: "Kopiert",
  },

  faq: {
    kicker: "FAQ",
    title: "Fragen, beantwortet.",
    lead: "Alles, was du wissen musst, bevor du deinen ersten animierten Hintergrund unter Linux setzt.",
    items: [
      {
        q: "Gibt es eine Wallpaper Engine für Linux?",
        a: "Ja. Fresco ist eine kostenlose Open-Source-App für animierte Hintergründe unter Linux, die wie Wallpaper Engine funktioniert: Video, GIF oder Bild auswählen und als bewegten Desktophintergrund setzen. Sie ist auf die grafische Oberfläche ausgelegt und braucht weder Steam noch Proton.",
      },
      {
        q: "Wie setze ich unter Ubuntu oder Pop!_OS ein Video als Hintergrund?",
        a: "Installiere das Fresco-.deb, öffne es aus dem Anwendungsmenü, klick auf Hinzufügen, wähl dein Video, schneide oder drehe es bei Bedarf und klick auf Als Hintergrund setzen. Schließ die App, und das Video läuft weiter als Desktophintergrund.",
      },
      {
        q: "Zieht ein Videohintergrund CPU oder Akku leer?",
        a: "Nein. Fresco dekodiert Video über mpv auf der GPU (VA-API und NVDEC), die CPU-Last bleibt also nahe null und der Speicherbedarf liegt bei etwa 120 bis 150 MB. Im Akkubetrieb kann automatisch pausiert werden, und auf jedem Monitor mit einem Vollbildfenster pausiert Fresco von selbst.",
      },
      {
        q: "Funktioniert Fresco unter Wayland und auf dem COSMIC-Desktop?",
        a: "Ja. Fresco spielt animierte Hintergründe auf Wayland-Compositoren mit layer-shell über ein mitgeliefertes, überwachtes mpvpaper-Backend: COSMIC (Pop!_OS 24.04), Hyprland, Sway, KDE Plasma 6 und weitere wlroots-Compositoren. Seit v1.1.1 werden zwei mpvpaper-Builds ausgeliefert und zur Laufzeit geprüft, sodass es sowohl auf libmpv1- als auch auf libmpv2-Distributionen läuft. Unter X11 funktioniert es auf jedem Desktop.",
      },
      {
        q: "Funktioniert Fresco unter GNOME?",
        a: "Unter GNOME mit X11-Sitzung ja, mit vollständig animierten Hintergründen. Unter GNOME mit Wayland stellt Mutter keine Fläche für animierte Hintergründe bereit, deshalb zeigt Fresco ein Standbild des gewählten Hintergrunds, statt eine Animation vorzutäuschen.",
      },
      {
        q: "Kann ein Videohintergrund Ton wiedergeben?",
        a: "Ja. Jeder Hintergrund merkt sich seinen eigenen Stummschaltungsstatus und seine Lautstärke, du kannst also gezielt ein Video laut stellen, und die Wahl gilt jedes Mal, wenn es gesetzt wird. Standardmäßig starten Hintergründe stumm.",
      },
      {
        q: "Kann ich einen Hintergrund zuschneiden oder drehen?",
        a: "Ja. Der Editor hat einen Zuschneiderahmen zum Ziehen und eine 90-Grad-Drehung, du kannst also genau den gewünschten Ausschnitt wählen oder ein quer aufgenommenes Handyvideo aufrichten. Beides wird auf der GPU angewendet und pro Hintergrund gespeichert.",
      },
      {
        q: "Bleibt der Hintergrund nach einem Neustart erhalten?",
        a: "Ja. Fresco legt einen Autostart-Eintrag an, der deinen animierten Hintergrund beim Anmelden wiederherstellt, und repariert den Eintrag selbst, falls er fehlt. In den Einstellungen lässt sich das abschalten.",
      },
      {
        q: "Welche Medienformate werden unterstützt?",
        a: "Video in Schleife (mp4, webm, mkv, avi, mov), animierte GIFs, statische Bilder (jpg, png, webp), ein Bildordner als Diashow mit Überblendung, Ausblenden, Schieben oder Ken Burns, sowie Playlists mit mehreren Videos.",
      },
      {
        q: "Werden mehrere Monitore unterstützt?",
        a: "Ja. Du kannst auf jedem Bildschirm einen anderen Hintergrund setzen, und Fresco pausiert den Hintergrund pro Ausgang, sobald dort ein Fenster in den Vollbildmodus geht. Monitor-Hotplug wirkt unter X11 sofort; unter Wayland wird ein neu angeschlossener Bildschirm beim nächsten Anwenden erkannt (automatisches Hotplug kommt mit der v1.0-Engine).",
      },
      {
        q: "Worin unterscheidet sich Fresco von Wallpaper Engine?",
        a: "Wallpaper Engine ist ein kostenpflichtiges, primär für Windows entwickeltes Produkt, das unter Linux nur über Steam Play und Proton läuft. Fresco ist kostenlos, quelloffen (GPL-3.0) und nativ für Linux: kein Steam, kein Proton, keine Kompatibilitätsschicht. Statt des Steam Workshops gibt es einen eingebauten Katalog kuratierter, lizenzierter Hintergründe, und X11 sowie Wayland-Compositoren mit layer-shell werden direkt unterstützt.",
      },
      {
        q: "Worin unterscheidet sich Fresco von Hidamari, Komorebi und mpvpaper?",
        a: "Fresco ist auf die grafische Oberfläche ausgelegt, hardwarebeschleunigt und verwaltet Video-, GIF-, Bild-, Diashow- und Playlist-Hintergründe in einer App, unter X11 wie unter Wayland. Es wird aktiv gepflegt, anders als Komorebi, und braucht keine Kommandozeile, anders als mpvpaper.",
      },
      {
        q: "Wo finde ich animierte Hintergründe für Linux?",
        a: "In Fresco selbst. Der eingebaute Katalog (Menü, dann Hintergründe durchsuchen) bietet kuratierte, sauber lizenzierte Videohintergründe, die du mit zwei Klicks setzt, mit Lizenz und Urheber an jedem Eintrag. Du kannst auch eine direkte Video- oder Bild-URL einfügen oder eigene Dateien hinzufügen.",
      },
      {
        q: "Kann mein Hintergrund automatisch zwischen Tag und Nacht wechseln?",
        a: "Ja. Öffne das Menü, wähle Erweitert und dann Tag- und Nachthintergrund: zwei Hintergründe und zwei Wechselzeiten festlegen, und der Daemon tauscht sie ohne Neustart automatisch. Beliebige Zeitfenster sowie ein Wechsel zu Sonnenauf- oder -untergang (mit manuellen Koordinaten) sind über die config.toml verfügbar.",
      },
      {
        q: "Wie setze ich auf jedem Monitor einen anderen Hintergrund?",
        a: "Rechtsklick auf einen Hintergrund in der Bibliothek, dann Auf einem bestimmten Bildschirm setzen. Jeder angeschlossene Monitor wird mit seiner Auflösung aufgeführt. Mit Standard auf allen Bildschirmen zeigen werden die Einstellungen pro Monitor wieder entfernt.",
      },
      {
        q: "Kann ich Songtexte auf meinem Linux-Desktop anzeigen?",
        a: "Ja. Fresco zeichnet zeitsynchrone Songtexte auf deinen Hintergrund und folgt dabei allem, was auf deinem System über MPRIS läuft: Browser, Musik-Apps, Videoplayer. Es gibt vier Voreinstellungen, ein Raster mit neun Positionen, einen Regler für den Sync-Versatz, optional die nächste Zeile sowie optional Titel und Interpret. Der Text kommt zuerst aus einer lokalen .lrc-Datei, dann von LRCLIB, einer kostenlosen, community-betriebenen Datenbank. Firefox ist der zuverlässigste Player; der native Linux-Client von Spotify meldet eine fehlerhafte Wiedergabeposition, Spotify im Browser funktioniert dagegen einwandfrei.",
      },
      {
        q: "Gibt es unter Linux Desktop-Widgets wie Conky?",
        a: "Ja, und Fresco bringt vier davon mit, die weder Panel noch Erweiterung noch Unterstützung deines Desktops brauchen: synchroner Songtext, eine Uhr mit sechs Designs, ein Audio-Visualizer mit fünf Stilen und das Cover des laufenden Titels auf einer drehenden Schallplatte. Sie werden in den Hintergrund selbst gezeichnet statt in ein Fenster, liegen also nie über deinen Fenstern, fangen nie einen Klick ab und funktionieren auch auf Desktops ohne eigene Widget-Ebene, darunter COSMIC, Hyprland und Sway. Alle vier sind standardmäßig aus. Anders als bei Conky gibt es noch keine Systemmonitor-Widgets, also keine Anzeigen für CPU, RAM oder Netzwerk. GNOME unter Wayland ist der einzige Ort, an dem die Widgets nicht laufen können, weil dort keine Fläche für animierte Hintergründe zum Zeichnen existiert.",
      },
      {
        q: "Kann ich einen Musik-Visualizer auf dem Desktophintergrund bekommen?",
        a: "Ja. Frescos Audio-Visualizer reagiert auf alles, was dein System abspielt, in einem von fünf Stilen (Bars, Mirror, Wave, Dots, Ring) mit Farbwähler, Zweifarbverlauf oder Regenbogen. Er ist standardmäßig aus und fragt beim ersten Aktivieren um Zustimmung, weil er deine Audioausgabe mithören muss. Mit laufender Musik und allen vier aktivierten Widgets lag der gemessene Aufwand bei 0,8 % eines CPU-Kerns, fast vollständig durch die Audioaufnahme, denn neu gezeichnet wird nur, was sich inhaltlich geändert hat.",
      },
      {
        q: "Gibt es Fresco im deepin App Store?",
        a: "Ja. Öffne unter deepin 25 den App Store, suche nach Fresco und klicke auf Installieren. Fresco ist im deepin Community App Store veröffentlicht, daher kommen Updates über den Store. Das .deb und der Einzeiler-Installer funktionieren ebenfalls.",
      },
      {
        q: "Ist Fresco kostenlos?",
        a: "Ja. Fresco ist vollständig kostenlos und quelloffen unter der GPL-3.0-Lizenz. Es gibt keine kostenpflichtige Variante.",
      },
    ],
  },

  footer: {
    github: "GitHub",
    license: "Lizenz",
    tagline: "Rust + GTK4 + mpv",
    sound: "Ton umschalten",
  },

  featureList: [
    "Eingebauter Katalog kuratierter, lizenzierter Hintergründe",
    "Video-, GIF-, Bild-, Diashow- und Playlist-Hintergründe",
    "In den Hintergrund gezeichnete Widgets: synchroner Songtext, Uhr, Audio-Visualizer, Albumcover",
    "Hintergründe über eine direkte URL hinzufügen",
    "Tag- und Nachtzeitpläne (dazu Zeitfenster und Sonnenstand über die Konfiguration)",
    "Hintergrund pro Bildschirm über die Oberfläche",
    "Automatische Audiowiederherstellung, wenn der Soundserver spät startet",
    "Skriptbarer JSON-Steuersocket",
    "Hardwarebeschleunigte Wiedergabe (VA-API, NVDEC)",
    "Läuft unter X11 und auf Wayland-Compositoren mit layer-shell",
    "Editor zum Zuschneiden per Ziehen und 90-Grad-Drehen",
    "Ton und Lautstärke pro Hintergrund",
    "Diashow-Übergänge (Überblendung, Ausblenden, Schieben, Ken Burns)",
    "Hintergrundbibliothek mit Suche",
    "Unterschiedlicher Hintergrund pro Monitor",
    "Pause im Akkubetrieb und automatische Pause bei Vollbild",
    "Stellt sich beim Anmelden automatisch wieder her",
    "Designs und Akzentfarben",
  ],

  softwareDescription:
    "Fresco ist eine kostenlose Open-Source-App für animierte Hintergründe unter Linux. Sie setzt Video-, GIF-, Bild-, Diashow- und Playlist-Hintergründe als bewegten Desktophintergrund mit hardwarebeschleunigter Wiedergabe und kann vier Desktop-Widgets in den Hintergrund selbst zeichnen: zeitsynchronen Songtext, eine Uhr, einen Audio-Visualizer und das Albumcover auf einer drehenden Schallplatte. Eine kostenlose Wallpaper-Engine-Alternative für Pop!_OS, Ubuntu, Linux Mint, Debian und elementary OS, unter X11 und auf Wayland-Compositoren mit layer-shell (COSMIC, Hyprland, Sway, KDE Plasma 6).",
};
