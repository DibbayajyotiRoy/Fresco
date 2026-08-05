import type { Dictionary } from "./en";

/**
 * Français. Les noms de produit et les termes de plateforme restent en
 * anglais (Fresco, X11, Wayland, layer-shell, mpv, GPL-3.0 ...), comme dans
 * la documentation Linux francophone. Tutoiement, comme la version anglaise.
 */
export const fr: Dictionary = {
  meta: {
    title:
      "Fresco - Fond d'écran animé pour Linux | Alternative gratuite à Wallpaper Engine",
    description:
      "Application gratuite et open source de fonds d'écran animés pour Linux. Catalogue intégré, vidéos ou GIF sur le bureau, fond par écran, bascule automatique jour et nuit. Accélération matérielle sous X11 et Wayland.",
    ogTitle: "Fresco - Fonds d'écran animés pour Linux",
    ogDescription:
      "Catalogue intégré, fond par écran, planification jour et nuit et lecture accélérée par le matériel avec un processeur quasi inactif, sous X11 et Wayland. Une alternative gratuite à Wallpaper Engine.",
    twitterDescription:
      "Des fonds d'écran animés pour Linux, accélérés par le matériel, sous X11 et Wayland. Une alternative gratuite et open source à Wallpaper Engine.",
    ogImageAlt:
      "Fresco. Enfin un fond d'écran Linux qui fonctionne, tout simplement.",
    keywords: [
      "fond d'ecran anime linux",
      "fond d'ecran video linux",
      "wallpaper anime ubuntu",
      "alternative wallpaper engine linux",
    ],
  },

  nav: {
    home: "Accueil Fresco",
    features: "Fonctionnalités",
    compare: "Comparer",
    whatsNew: "Nouveautés",
    download: "Télécharger",
    cta: "Obtenir Fresco",
    star: "Mettre une étoile à Fresco sur GitHub",
    starWithCount: (n: string) =>
      `Mettre une étoile à Fresco sur GitHub (${n} étoiles)`,
  },

  language: {
    label: "Langue",
    change: "Changer de langue",
  },

  theme: {
    toggle: "Changer de thème",
    light: "Clair",
    dark: "Sombre",
  },

  hero: {
    titleLead: "Enfin un fond d'écran Linux",
    /** Separator before the accented tail; empty where CJK needs none. */
    titleGap: " ",
    titleEm: "qui fonctionne, tout simplement.",
    body: "Mets n'importe quelle vidéo, GIF ou image sur ton bureau Linux. La lecture accélérée par le matériel garde le processeur quasi inactif, sous X11 comme sous Wayland. Ferme l'application : le démon continue la lecture.",
    install: "Installer Fresco",
    star: "Étoile sur GitHub",
  },

  stats: {
    ariaLabel: "Statistiques du projet",
    downloads: "téléchargements au total",
    downloadsUnknown: "téléchargements sur github",
    stars: "étoiles github",
    version: "dernière version",
    license: "gratuit et open source",
  },

  glance: {
    ariaLabel: "Fresco en bref",
    caption: "fresco en bref",
    labelWhat: "c'est quoi",
    labelPlatforms: "plateformes",
    labelWidgets: "widgets",
    labelLicense: "licence",
    labelInstall: "installation",
    what: "Fresco est une application gratuite et open source de fonds d'écran animés pour Linux : elle met des vidéos, des GIF, des images, des diaporamas et des playlists en fond de bureau animé, avec décodage matériel sur le GPU. Une alternative gratuite à Wallpaper Engine, et une interface graphique pour mpvpaper sous Wayland.",
    platforms:
      "N'importe quel bureau X11 (Ubuntu, Pop!_OS, Linux Mint, Debian), ainsi que les compositeurs Wayland avec layer-shell : COSMIC, Hyprland, Sway, KDE Plasma 6. Sous GNOME avec Wayland, repli sur une image fixe.",
    widgets:
      "Quatre widgets dessinés dans le fond d'écran lui-même, pas dans une fenêtre : paroles synchronisées, une horloge à six thèmes, un visualiseur audio et la pochette de l'album sur un disque qui tourne. Rien ne flotte au-dessus de tes fenêtres et rien n'intercepte un clic. Tous désactivés par défaut. Indisponibles sous GNOME avec Wayland, qui n'offre pas de surface de fond animé.",
    licenseLead: "GPL-3.0, gratuit pour toujours.",
    licenseLink: "Code source sur GitHub",
    licenseTail: "Construit avec Rust, GTK4 et mpv.",
  },

  features: {
    kicker: "fonctionnalités",
    title: "N'importe quel média. N'importe quel écran. Sans drame côté CPU.",
    lead: "Fresco met des fonds d'écran vidéo, GIF, image, diaporama et playlist sous X11 et Wayland, décodés sur le GPU, si bien qu'un fond animé coûte à peu près autant qu'un fond fixe. La fiche technique complète :",
    manifest: (n: number) => `manifeste : ${n} fonctionnalités`,
    /** Sentence-final mark after each row title. */
    titleSuffix: ".",
    thCapability: "Fonctionnalité",
    thWhatYouGet: "Ce que tu obtiens",
    thStatus: "Statut",
    footnote:
      "gnome wayland : repli sur une image fixe (mutter n'expose pas de surface animée), et les widgets ont besoin de cette même surface, donc ils n'y sont pas disponibles. tout le reste ci-dessus fonctionne.",
    tally: (shipping: number, total: number, soon: number) =>
      `${shipping} sur ${total} disponibles · ${soon} en préversion · 0 abandonnée`,
    rows: {
      hwDecode: {
        tag: "décodage matériel",
        title: "Lecture accélérée par le matériel",
        description:
          "Le décodage tourne sur le GPU via mpv (VA-API ou NVDEC). Un fond d'écran vidéo en 4K coûte à peu près autant de CPU qu'une image fixe.",
        status: "cpu quasi nul",
      },
      sessions: {
        tag: "sessions",
        title: "X11 et Wayland",
        description:
          "Un backend fenêtre de bureau sur n'importe quelle session X11, plus un backend layer-shell pour COSMIC, Hyprland, Sway et KDE Plasma 6. GNOME avec Wayland reçoit une image fixe.",
        status: "x11 · layer-shell",
      },
      catalog: {
        tag: "catalogue",
        title: "Catalogue de fonds d'écran intégré",
        description:
          "Parcours des fonds d'écran sélectionnés et sous licence directement dans l'application (menu, puis Parcourir les fonds d'écran) et applique-en un en deux clics. Tu peux aussi coller un lien direct.",
        status: "dans l'app",
      },
      video: {
        tag: "vidéo · gif",
        title: "Fonds d'écran vidéo et GIF",
        description:
          "Lis en boucle n'importe quel mp4, webm, mkv ou GIF animé sur ton bureau.",
        status: "mp4 webm mkv gif",
      },
      slideshow: {
        tag: "diaporama",
        title: "Diaporamas avec transitions",
        description:
          "Fais défiler un dossier d'images en fondu enchaîné, fondu ou Ken Burns.",
        status: "4 transitions",
      },
      playlist: {
        tag: "playlist",
        title: "Playlists vidéo",
        description:
          "Mets plusieurs clips à la suite et laisse Fresco les enchaîner.",
        status: "cycle automatique",
      },
      lyrics: {
        tag: "paroles · horloge",
        title: "Widgets paroles et horloge",
        description:
          "Des paroles synchronisées avec ce qui passe via MPRIS (d'abord le .lrc local, puis LRCLIB), et une horloge parmi six thèmes. Dessinées dans le fond d'écran, donc rien ne flotte au-dessus de tes fenêtres. Désactivés par défaut.",
        status: "désactivé par défaut",
      },
      visualiser: {
        tag: "visualiseur",
        title: "Visualiseur audio et pochette",
        description:
          "Cinq styles (Bars, Mirror, Wave, Dots, Ring) avec sélecteur de couleur, dégradé à deux couleurs ou arc-en-ciel, plus la pochette du morceau en cours sur un disque qui tourne. Le visualiseur demande avant d'écouter ton audio.",
        status: "0,8 % d'un cœur",
      },
      editor: {
        tag: "éditeur",
        title: "Recadrer et pivoter",
        description:
          "Fais glisser un cadre pour choisir la zone, pivote de 90 degrés pour redresser un clip filmé de travers. Les deux restent zero-copy sur le GPU.",
        status: "zero-copy",
      },
      audio: {
        tag: "audio",
        title: "Son par fond d'écran",
        description:
          "Réactive le son d'une vidéo et règle son volume. Fresco retient le choix pour ce fond d'écran.",
        status: "par fond d'écran",
      },
      displays: {
        tag: "écrans",
        title: "Fond d'écran par écran",
        description:
          "Clic droit sur un fond d'écran, puis Appliquer à un écran précis. Chaque moniteur peut avoir le sien.",
        status: "par moniteur",
      },
      schedule: {
        tag: "planification",
        title: "Planification jour et nuit",
        description:
          "Deux fonds d'écran, deux heures de bascule, échangés automatiquement par le démon. Plages horaires et bascule solaire via la configuration.",
        status: "automatique",
      },
      power: {
        tag: "énergie",
        title: "Attentif à l'énergie",
        description:
          "Pause sur batterie, et pause automatique par moniteur dès qu'une fenêtre y passe en plein écran.",
        status: "pause auto",
      },
      newTab: {
        tag: "nouvel onglet",
        title: "Ton fond d'écran à chaque nouvel onglet",
        description:
          "Une extension de navigateur (Chrome, Brave, Edge, Firefox) reprend le fond d'écran de ton bureau, ou un choix propre au navigateur, sur la page de nouvel onglet, via un pont local qui ne parle qu'à 127.0.0.1. Déjà dans le dépôt ; la publication sur les stores est en attente.",
        status: "bientôt",
      },
      themes: {
        tag: "thèmes",
        title: "Thèmes et couleurs d'accent",
        description:
          "Clair, sombre ou selon le système, avec six palettes d'accent.",
        status: "6 palettes",
      },
    },
  },

  compare: {
    kicker: "comparer",
    title: "Fresco face aux fonds d'écran animés sous Linux.",
    lead: "Fresco est la seule application de fond d'écran animé pour Linux de ce tableau à réunir une interface graphique, le décodage matériel, la prise en charge de X11 et de Wayland et un catalogue intégré, gratuitement et avec une maintenance active. Voici la comparaison complète avec Hidamari, Komorebi, mpvpaper et Wallpaper Engine.",
    meter: (tools: number, caps: number) =>
      `comparaison · ${tools} outils · ${caps} fonctionnalités`,
    thFeature: "Fonctionnalité",
    yes: "Oui",
    no: "Non",
    note: "Wallpaper Engine est un produit payant conçu d'abord pour Windows. Komorebi n'est plus maintenu.",
    detailLabel: "Comparer en détail :",
    vs: (tool: string) => `Fresco vs ${tool}`,
    rows: {
      gui: "Application graphique, sans terminal",
      x11: "Fonctionne sous X11",
      wayland: "Fonctionne sous Wayland (layer-shell)",
      hwDecode: "Décodage matériel, CPU faible",
      cropRotate: "Recadrage par glisser et rotation",
      playlists: "Playlists",
      slideshow: "Diaporama d'images",
      library: "Bibliothèque de fonds d'écran",
      catalog: "Catalogue intégré",
      perDisplay: "Fond d'écran par écran (interface)",
      schedules: "Planification jour et nuit",
      maintained: "Maintenance active",
      foss: "Gratuit et open source",
    },
    cells: {
      partial: "Partiel",
      manual: "Manuel",
      compositorOff: "Sans compositeur",
      cropOnly: "Recadrage seul",
      workshop: "Workshop",
    },
  },

  whatsNew: {
    kicker: (version: string) => `nouveautés · v${version}`,
    title: "Quatre widgets de bureau, peints dans le fond d'écran.",
    lead: (version: string) =>
      `Ce qui est arrivé en v${version}. Aucune fenêtre en plus, rien à cliquer, identique sous X11 et sous layer-shell. Les quatre sont désactivés par défaut et, musique en cours et tous activés, le coût mesuré était de 0,8 % d'un cœur de processeur. Chaque entrée ici est reprise dans le CHANGELOG sur GitHub.`,
    changelog: "Changelog complet",
    patch: (n: string) => `patch ${n}`,
    items: {
      lyrics: {
        title: "Paroles synchronisées",
        body: "La ligne en cours, en rythme avec ce qui passe via MPRIS. D'abord les fichiers .lrc locaux, puis LRCLIB. Quatre préréglages et un décalage de synchronisation.",
      },
      clock: {
        title: "Horloge, six thèmes",
        body: "Digital, Minimal, Segment, Stacked, Wordy et Card, un panneau translucide avec un cadran analogique dessiné. 12 ou 24 heures, date facultative.",
      },
      visualizer: {
        title: "Visualiseur audio",
        body: "Bars, Mirror, Wave, Dots ou Ring, avec sélecteur de couleur, dégradé à deux couleurs ou arc-en-ciel. Demande avant d'écouter ton audio.",
      },
      disc: {
        title: "Pochette sur un disque",
        body: "La pochette du morceau en cours sur un disque qui tourne. Il s'arrête à l'instant où la lecture se met en pause.",
      },
    },
  },

  howItWorks: {
    kicker: "comment ça marche",
    title: "Trois clics, puis on oublie.",
    lead: "Ouvre Fresco, clique sur ajouter, clique sur appliquer, ferme. Le démon garde le fond d'écran en marche, même après un redémarrage.",
    step: (n: string) => `étape ${n}`,
    steps: {
      pick: {
        title: "Choisis ton média",
        description:
          "Ouvre Fresco depuis le menu des applications et choisis une vidéo, un GIF, une image, un dossier ou une playlist.",
      },
      set: {
        title: "Clique sur Appliquer",
        description:
          "Applique-le comme fond d'écran. La lecture démarre aussitôt sur ton bureau.",
      },
      close: {
        title: "Ferme l'application",
        description:
          "Ferme la fenêtre. Un démon léger garde le fond d'écran en marche, même après un redémarrage.",
      },
    },
  },

  videos: {
    kicker: "en fonctionnement",
    title: "Moins d'une minute chacune. Sans commentaire.",
    lead: "De courtes captures d'écran de Fresco sur un vrai bureau. Rien n'est chargé depuis YouTube tant que tu n'as pas appuyé sur lecture.",
    more: "Plus sur YouTube",
    inDevelopment: "en développement",
    play: (title: string) => `Lire : ${title}`,
    items: {
      "YWzD3-xkCEc": {
        tag: "ajouter par lien",
        blurb:
          "Copie un lien Pinterest, colle-le dans Fresco, applique-le en fond d'écran. Aucun téléchargement, aucune gymnastique de fichiers.",
      },
      C1MqrhGkovQ: {
        tag: "widgets paroles",
        blurb:
          "Paroles synchronisées et horloge dessinées dans un fond d'écran animé, sous Wayland et X11. Arrivées en v1.1.36, avec un visualiseur audio et un disque de pochette.",
      },
    },
  },

  supported: {
    kicker: "environnements testés",
    title: "Où tourne Fresco.",
    lead: "Sur n'importe quel bureau X11, y compris le DDE de Deepin 25, et sur les compositeurs Wayland avec layer-shell (COSMIC, Hyprland, Sway et KDE Plasma 6), à travers les distributions Debian et Ubuntu les plus répandues. GNOME avec Wayland reçoit une image fixe.",
    deployed: (distros: number, formats: number) =>
      `testé : 6 compositeurs animés · 1 repli fixe · ${distros} distributions · ${formats} formats`,
    sessionsTitle: "sessions et compositeurs",
    distrosTitle: (n: number) => `distributions testées · ${n}`,
    formatsTitle: (n: number) => `formats pris en charge · ${n}`,
    live: "Fond d'écran animé",
    fallback: "Image fixe",
    sessions: {
      x11: {
        label: "X11 (tous les bureaux)",
        detail: "GNOME, KDE, XFCE, MATE, Cinnamon, Budgie",
      },
      deepin: {
        label: "Deepin 25 (DDE, X11)",
        detail:
          "Adaptation automatique à DDE, les icônes restent visibles. Vérifié par la communauté sur Deepin 25 Community build1.",
      },
      wayland: {
        label: "Wayland layer-shell",
        detail: "COSMIC, Hyprland, Sway, KDE Plasma 6, wlroots",
      },
      gnome: {
        label: "GNOME sous Wayland",
        detail: "Repli sur une image fixe (Mutter n'a pas de surface animée)",
      },
    },
    fieldReport: "retour de terrain · deepin 25",
    verifiedEnv: "environnement vérifié",
    testimonialRole: "Testeur de la communauté Deepin",
    envLabels: {
      session: "session",
      os: "os",
      gpu: "gpu",
    },
    footnote:
      "deepin 25 utilise x11 par défaut, et c'est la session sur laquelle fresco y est vérifié. treeland, le compositeur wayland propre à deepin, est encore en développement, donc fresco n'affirme rien pour l'instant sur deepin sous wayland.",
  },

  download: {
    kicker: "télécharger",
    title: "À déployer sur Debian, Ubuntu, Pop!_OS et Mint.",
    badge: "x11 · wayland",
    lead: "L'installeur officiel en une ligne ou le paquet .deb. Les deux chemins se copient dans ton presse-papiers et s'exécutent immédiatement. Fresco continue de lire après la fermeture de la fenêtre.",
    cardTitle: "installation en une ligne",
    cardBody:
      "Lance ceci dans un terminal. Il télécharge et installe le dernier .deb pour toi, toujours la version la plus récente :",
    terminalTitle: "fresco install",
    aptComment: "tu as déjà téléchargé le .deb ?",
    releases: "Voir toutes les versions",
    gpuNote:
      "Pour l'usage processeur le plus bas, installe le pilote de décodage matériel de ton GPU (pilote Intel media VA, pilotes VA de Mesa, ou le pilote propriétaire NVIDIA pour NVDEC).",
    copy: "Copier",
    copied: "Copié",
  },

  faq: {
    kicker: "faq",
    title: "Vos questions, nos réponses.",
    lead: "Tout ce qu'il faut savoir avant d'appliquer ton premier fond d'écran animé sous Linux.",
    items: [
      {
        q: "Existe-t-il un Wallpaper Engine pour Linux ?",
        a: "Oui. Fresco est une application gratuite et open source de fond d'écran animé pour Linux qui fonctionne comme Wallpaper Engine : choisis une vidéo, un GIF ou une image et applique-la comme fond de bureau animé. Elle est pensée pour l'interface graphique et ne demande ni Steam ni Proton.",
      },
      {
        q: "Comment mettre une vidéo en fond d'écran sur Ubuntu ou Pop!_OS ?",
        a: "Installe le .deb de Fresco, ouvre-le depuis le menu des applications, clique sur Ajouter, choisis ta vidéo, recadre-la ou pivote-la si tu veux, puis clique sur Appliquer comme fond d'écran. Ferme l'application et la vidéo continue de tourner en fond de bureau.",
      },
      {
        q: "Un fond d'écran vidéo épuise-t-il le processeur ou la batterie ?",
        a: "Non. Fresco décode la vidéo sur le GPU via mpv (VA-API et NVDEC), l'usage processeur reste donc quasi nul et la mémoire tourne autour de 120 à 150 Mo. Il peut se mettre en pause automatiquement sur batterie, et il se met en pause tout seul sur tout écran comportant une fenêtre en plein écran.",
      },
      {
        q: "Fresco fonctionne-t-il sous Wayland et sur le bureau COSMIC ?",
        a: "Oui. Fresco fait tourner des fonds d'écran animés sur les compositeurs Wayland avec layer-shell grâce à un backend mpvpaper embarqué et supervisé : COSMIC (Pop!_OS 24.04), Hyprland, Sway, KDE Plasma 6 et les autres compositeurs wlroots. Depuis la v1.1.1, deux builds de mpvpaper sont livrées et testées à l'exécution, ce qui le rend compatible avec les distributions en libmpv1 comme en libmpv2. Sous X11, il fonctionne sur tous les bureaux.",
      },
      {
        q: "Fresco fonctionne-t-il sous GNOME ?",
        a: "Sous GNOME en session X11, oui, avec des fonds d'écran animés complets. Sous GNOME avec Wayland, Mutter n'expose pas de surface de fond animé : Fresco affiche donc une image fixe du fond choisi plutôt que de faire semblant d'animer.",
      },
      {
        q: "Un fond d'écran vidéo peut-il émettre du son ?",
        a: "Oui. Chaque fond d'écran retient son propre état de sourdine et son volume : tu peux réactiver le son d'une vidéo précise et le choix tient à chaque fois qu'elle est appliquée. Par défaut, les fonds d'écran démarrent en sourdine.",
      },
      {
        q: "Puis-je recadrer ou pivoter un fond d'écran ?",
        a: "Oui. L'éditeur propose un cadre de recadrage par glisser et une rotation de 90 degrés : tu peux choisir la zone exacte ou redresser une vidéo de téléphone filmée de travers. Les deux sont appliqués sur le GPU et retenus par fond d'écran.",
      },
      {
        q: "Le fond d'écran reste-t-il après un redémarrage ?",
        a: "Oui. Fresco ajoute une entrée de démarrage automatique qui restaure ton fond d'écran animé à la connexion, et répare cette entrée d'elle-même si elle disparaît. Tu peux désactiver cela dans les réglages.",
      },
      {
        q: "Quels formats de média sont pris en charge ?",
        a: "Vidéo en boucle (mp4, webm, mkv, avi, mov), GIF animés, images fixes (jpg, png, webp), un dossier d'images en diaporama avec transitions fondu enchaîné, fondu, glissement ou Ken Burns, et des playlists de plusieurs vidéos.",
      },
      {
        q: "Prend-il en charge plusieurs écrans ?",
        a: "Oui. Tu peux appliquer un fond d'écran différent sur chaque écran, et Fresco met en pause le fond de cette sortie dès qu'une fenêtre y passe en plein écran. Le branchement à chaud d'un moniteur est immédiat sous X11 ; sous Wayland, un écran fraîchement branché est pris en compte à l'application suivante (la détection automatique arrive avec le moteur v1.0).",
      },
      {
        q: "En quoi Fresco diffère-t-il de Wallpaper Engine ?",
        a: "Wallpaper Engine est un produit payant conçu d'abord pour Windows, qui ne tourne sous Linux qu'à travers Steam Play et Proton. Fresco est gratuit, open source (GPL-3.0) et natif sous Linux : ni Steam, ni Proton, ni couche de compatibilité. À la place du Steam Workshop, il propose un catalogue intégré de fonds d'écran sélectionnés et sous licence, et il prend directement en charge X11 et les compositeurs Wayland avec layer-shell.",
      },
      {
        q: "En quoi Fresco diffère-t-il de Hidamari, Komorebi et mpvpaper ?",
        a: "Fresco est pensé pour l'interface graphique, accéléré par le matériel, et gère les fonds d'écran vidéo, GIF, image, diaporama et playlist dans une seule application, sous X11 comme sous Wayland. Il est activement maintenu, contrairement à Komorebi, et ne demande aucune ligne de commande, contrairement à mpvpaper.",
      },
      {
        q: "Où trouver des fonds d'écran animés pour Linux ?",
        a: "Dans Fresco lui-même. Le catalogue intégré (menu, puis Parcourir les fonds d'écran) propose des fonds d'écran vidéo sélectionnés et correctement licenciés, applicables en deux clics, avec la licence et l'auteur affichés sur chaque élément. Tu peux aussi coller l'URL directe d'une vidéo ou d'une image, ou ajouter tes propres fichiers.",
      },
      {
        q: "Mon fond d'écran peut-il changer automatiquement entre le jour et la nuit ?",
        a: "Oui. Ouvre le menu, choisis Avancé puis Fond d'écran jour et nuit : sélectionne deux fonds d'écran et les heures de bascule, et le démon les échange automatiquement sans redémarrage. Des plages horaires libres et une bascule au lever ou au coucher du soleil (avec coordonnées manuelles) sont disponibles via config.toml.",
      },
      {
        q: "Comment mettre un fond d'écran différent sur chaque écran ?",
        a: "Fais un clic droit sur un fond d'écran dans la bibliothèque et choisis Appliquer à un écran précis. Chaque moniteur connecté est listé avec sa résolution. Choisir Afficher le fond par défaut sur tous les écrans efface les réglages par moniteur.",
      },
      {
        q: "Puis-je afficher les paroles d'une chanson sur mon bureau Linux ?",
        a: "Oui. Fresco dessine des paroles synchronisées sur ton fond d'écran, en suivant ce qui joue sur ton système via MPRIS : navigateurs, applications musicales, lecteurs vidéo. Il y a quatre préréglages, une grille de neuf positions, un curseur de décalage de synchronisation, une ligne suivante facultative, ainsi que le titre et l'artiste en option. Les paroles viennent d'abord d'un fichier .lrc local, puis de LRCLIB, une base gratuite gérée par la communauté. Firefox est le lecteur le plus fiable ; le client Linux natif de Spotify rapporte une position de lecture erronée, mais Spotify dans un navigateur fonctionne très bien.",
      },
      {
        q: "Linux a-t-il des widgets de bureau comme Conky ?",
        a: "Oui, et Fresco en ajoute quatre qui ne demandent ni tableau de bord, ni extension, ni prise en charge de ton bureau : paroles synchronisées, une horloge à six thèmes, un visualiseur audio à cinq styles et la pochette du morceau en cours sur un disque qui tourne. Ils sont peints dans le fond d'écran lui-même plutôt que dans une fenêtre : ils ne passent jamais au-dessus de tes fenêtres, n'interceptent jamais un clic, et fonctionnent sur des bureaux sans couche de widgets propre, dont COSMIC, Hyprland et Sway. Les quatre sont désactivés par défaut. Contrairement à Conky, il n'y a pas encore de widgets de surveillance système, donc pas d'affichage du processeur, de la mémoire ni du réseau. GNOME sous Wayland est le seul endroit où les widgets ne peuvent pas tourner, faute de surface de fond animé sur laquelle dessiner.",
      },
      {
        q: "Puis-je avoir un visualiseur de musique sur mon fond de bureau ?",
        a: "Oui. Le visualiseur audio de Fresco réagit à ce que joue ton système, dans l'un des cinq styles (Bars, Mirror, Wave, Dots, Ring) avec sélecteur de couleur, dégradé à deux couleurs ou arc-en-ciel. Il est désactivé par défaut et demande ton accord la première fois que tu l'actives, car il doit écouter ta sortie audio. Musique en cours et les quatre widgets activés, le coût mesuré était de 0,8 % d'un cœur de processeur, presque entièrement dû à la capture audio, puisque rien n'est redessiné tant que le contenu ne change pas.",
      },
      {
        q: "Fresco est-il gratuit ?",
        a: "Oui. Fresco est entièrement gratuit et open source sous licence GPL-3.0. Il n'y a pas de version payante.",
      },
    ],
  },

  footer: {
    github: "GitHub",
    license: "Licence",
    tagline: "rust + gtk4 + mpv",
    sound: "Activer ou couper le son",
  },

  featureList: [
    "Catalogue intégré de fonds d'écran sélectionnés et sous licence",
    "Fonds d'écran vidéo, GIF, image, diaporama et playlist",
    "Widgets dessinés dans le fond d'écran : paroles synchronisées, horloge, visualiseur audio, pochette d'album",
    "Ajout de fonds d'écran depuis une URL directe",
    "Planification jour et nuit du fond d'écran (plus plages horaires et mode solaire via la configuration)",
    "Fond d'écran par écran depuis l'interface graphique",
    "Récupération audio automatique quand le serveur de son démarre tard",
    "Socket de contrôle JSON scriptable",
    "Lecture accélérée par le matériel (VA-API, NVDEC)",
    "Fonctionne sous X11 et sur les compositeurs Wayland avec layer-shell",
    "Éditeur de recadrage par glisser et rotation de 90 degrés",
    "Son et volume par fond d'écran",
    "Transitions de diaporama (fondu enchaîné, fondu, glissement, Ken Burns)",
    "Bibliothèque de fonds d'écran avec recherche",
    "Un fond d'écran différent par moniteur",
    "Pause sur batterie et pause automatique en plein écran",
    "Restauration automatique à la connexion",
    "Thèmes et couleurs d'accent",
  ],

  softwareDescription:
    "Fresco est une application gratuite et open source de fond d'écran animé pour Linux. Elle applique des fonds d'écran vidéo, GIF, image, diaporama et playlist comme fond de bureau animé, avec une lecture accélérée par le matériel, et peut dessiner quatre widgets dans le fond d'écran lui-même : paroles synchronisées, une horloge, un visualiseur audio et la pochette d'album sur un disque qui tourne. Une alternative gratuite à Wallpaper Engine pour Pop!_OS, Ubuntu, Linux Mint, Debian et elementary OS, sous X11 et sur les compositeurs Wayland avec layer-shell (COSMIC, Hyprland, Sway, KDE Plasma 6).",
};
