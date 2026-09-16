import type { Dictionary } from "./en";

/**
 * Português do Brasil. Nomes de produto e termos de plataforma permanecem em
 * inglês (Fresco, X11, Wayland, layer-shell, mpv, GPL-3.0 ...), como é padrão
 * na documentação Linux em português.
 */
export const ptBr: Dictionary = {
  meta: {
    title:
      "Fresco - Papel de parede animado para Linux | Alternativa gratuita ao Wallpaper Engine",
    description:
      "App gratuito e de código aberto de papel de parede animado para Linux. Catálogo embutido, vídeos ou GIFs na área de trabalho, papel de parede por monitor e troca automática entre dia e noite. Aceleração por hardware no X11 e no Wayland.",
    ogTitle: "Fresco - Papéis de parede animados para Linux",
    ogDescription:
      "Catálogo embutido, papel de parede por monitor, agendamento de dia e noite e reprodução acelerada por hardware com uso de CPU quase zero no X11 e no Wayland. Uma alternativa gratuita ao Wallpaper Engine.",
    twitterDescription:
      "Papéis de parede animados para Linux com aceleração por hardware, no X11 e no Wayland. Uma alternativa gratuita e de código aberto ao Wallpaper Engine.",
    ogImageAlt:
      "Fresco. Finalmente, um papel de parede para Linux que simplesmente funciona.",
    keywords: [
      "papel de parede animado linux",
      "papel de parede em video linux",
      "wallpaper animado ubuntu",
      "alternativa wallpaper engine linux",
    ],
  },

  nav: {
    home: "Início do Fresco",
    features: "Recursos",
    compare: "Comparar",
    whatsNew: "Novidades",
    download: "Baixar",
    cta: "Obter o Fresco",
    star: "Dar uma estrela ao Fresco no GitHub",
    starWithCount: (n: string) =>
      `Dar uma estrela ao Fresco no GitHub (${n} estrelas)`,
    menu: "Menu",
  },

  language: {
    label: "Idioma",
    change: "Mudar idioma",
  },

  theme: {
    toggle: "Alternar tema",
    light: "Claro",
    dark: "Escuro",
  },

  hero: {
    titleLead: "Finalmente, um papel de parede para Linux",
    /** Separator before the accented tail; empty where CJK needs none. */
    titleGap: " ",
    titleEm: "que simplesmente funciona.",
    body: "Coloque qualquer vídeo, GIF ou imagem na sua área de trabalho Linux. A reprodução acelerada por hardware mantém a CPU perto de zero, no X11 e no Wayland. Feche o app: o daemon continua reproduzindo.",
    install: "Instalar o Fresco",
    star: "Dar estrela no GitHub",
    newUsers24h: (n: string) =>
      `${n} pessoas começaram a usar o Fresco nas últimas 24 horas`,
    builtBy: "Criado por",
    activeToday: (n: string) =>
      `${n} pessoas usaram o Fresco nas últimas 24 horas`,
    newToday: (n: string) => `${n} novas`,
  },

  stats: {
    ariaLabel: "Estatísticas do projeto",
    downloads: "downloads totais",
    downloadsUnknown: "downloads no GitHub",
    stars: "estrelas no GitHub",
    version: "Versão mais recente",
    license: "gratuito e de código aberto",
    users: "usuários",
    countries: "países",
    cohortNote:
      "Usuários e países são contados a partir de instalações que aceitaram a telemetria anônima.",
    active24h: "ativos nas últimas 24 h",
  },

  testimonials: {
    kicker: "Relatos",
    title: (users: string, countries: string) =>
      `Rodando em ${users} desktops Linux em ${countries} países.`,
    lead: "Recados sem edição de quem usa o Fresco, enviados de dentro do app. Feedback anônimo é identificado apenas pelo país.",
    fromCountry: (country: string) => `Usuário do Fresco · ${country}`,
    anonymous: "Usuário do Fresco",
    namedLabel: "Relato de campo",
    pause: "Pausar relatos",
    play: "Reproduzir relatos",
  },

  glance: {
    ariaLabel: "Fresco em resumo",
    caption: "Fresco em resumo",
    labelWhat: "O que é",
    labelPlatforms: "Plataformas",
    labelWidgets: "Widgets",
    labelLicense: "Licença",
    labelInstall: "Instalação",
    what: "O Fresco é um app gratuito e de código aberto de papel de parede animado para Linux: ele define vídeos, GIFs, imagens, apresentações de slides e playlists como plano de fundo animado, com decodificação por hardware na GPU. Uma alternativa gratuita ao Wallpaper Engine e uma interface gráfica para o mpvpaper no Wayland.",
    platforms:
      "Qualquer área de trabalho X11 (Ubuntu, Pop!_OS, Linux Mint, Debian), além dos compositores Wayland com layer-shell: COSMIC, Hyprland, Sway e KDE Plasma 6. No GNOME com Wayland ele recorre a um quadro estático.",
    widgets:
      "Quatro widgets desenhados no próprio papel de parede, e não em uma janela: letras de música sincronizadas, um relógio com seis temas, um visualizador de áudio e a capa do álbum em um disco girando. Nada flutua sobre as suas janelas e nada intercepta cliques. Todos vêm desativados. Indisponíveis no GNOME com Wayland, que não tem superfície de papel de parede animado.",
    licenseLead: "GPL-3.0, gratuito para sempre.",
    licenseLink: "Código no GitHub",
    licenseTail: "Feito com Rust, GTK4 e mpv.",
  },

  features: {
    kicker: "Recursos",
    title: "Qualquer mídia. Qualquer monitor. Sem drama de CPU.",
    lead: "O Fresco define papéis de parede em vídeo, GIF, imagem, apresentação de slides e playlist no X11 e no Wayland, decodificados na GPU, de modo que um papel de parede animado custa quase o mesmo que um estático.",
    manifest: (n: number) => `Manifesto: ${n} recursos`,
    /** Sentence-final mark after each row title. */
    titleSuffix: ".",
    thCapability: "Recurso",
    thWhatYouGet: "O que você ganha",
    thStatus: "Status",
    footnote:
      "GNOME Wayland: recorre a um quadro estático (o Mutter não expõe superfície animada), e os widgets também precisam dessa superfície, então não estão disponíveis ali. Todo o resto acima funciona.",
    tally: (shipping: number, total: number, soon: number) =>
      `${shipping} de ${total} disponíveis · ${soon} em prévia · 0 descontinuados`,
    rows: {
      hwDecode: {
        tag: "Decodif. HW",
        title: "Reprodução acelerada por hardware",
        description:
          "A decodificação roda na GPU através do mpv (VA-API ou NVDEC). Um papel de parede em 4K custa quase a mesma CPU que uma imagem estática.",
        status: "CPU quase zero",
      },
      sessions: {
        tag: "Sessões",
        title: "X11 e Wayland",
        description:
          "Um backend de janela de área de trabalho em qualquer sessão X11, mais um backend layer-shell para COSMIC, Hyprland, Sway e KDE Plasma 6. O GNOME com Wayland recebe um quadro estático.",
        status: "X11 · layer-shell",
      },
      catalog: {
        tag: "Catálogo",
        title: "Catálogo de papéis de parede embutido",
        description:
          "Navegue por papéis de parede selecionados e licenciados dentro do app (menu, depois Procurar papéis de parede) e defina um em dois cliques. Você também pode colar um link direto.",
        status: "No app",
      },
      video: {
        tag: "Vídeo · GIF",
        title: "Papéis de parede em vídeo e GIF",
        description:
          "Reproduza em loop qualquer mp4, webm, mkv ou GIF animado na sua área de trabalho.",
        status: "mp4 webm mkv gif",
      },
      slideshow: {
        tag: "Slides",
        title: "Apresentações com transições",
        description:
          "Alterne uma pasta de imagens com crossfade, fade ou Ken Burns.",
        status: "4 transições",
      },
      playlist: {
        tag: "Playlist",
        title: "Playlists de vídeo",
        description:
          "Enfileire vários clipes e deixe o Fresco alternar entre eles.",
        status: "Ciclo automático",
      },
      lyrics: {
        tag: "Letras · relógio",
        title: "Widgets de letra e relógio",
        description:
          "Letras sincronizadas com o que estiver tocando via MPRIS (primeiro o .lrc local, depois o LRCLIB) e um relógio em um de seis temas. Desenhados no papel de parede, então nada flutua sobre as suas janelas. Desativados por padrão.",
        status: "Desativado por padrão",
      },
      visualiser: {
        tag: "Visualizador",
        title: "Visualizador de áudio e capa do álbum",
        description:
          "Cinco estilos (Bars, Mirror, Wave, Dots, Ring) com seletor de cor, mistura de duas cores ou arco-íris, mais a capa da faixa atual em um disco girando. O visualizador pede permissão antes de escutar o seu áudio.",
        status: "0,8% de um núcleo",
      },
      editor: {
        tag: "Editor",
        title: "Recortar e girar",
        description:
          "Arraste um quadro para escolher a região e gire 90 graus para corrigir clipes deitados. Ambos continuam zero-copy na GPU.",
        status: "Zero-copy",
      },
      audio: {
        tag: "Áudio",
        title: "Som por papel de parede",
        description:
          "Ative o som de um vídeo e ajuste o volume. O Fresco lembra a escolha para aquele papel de parede.",
        status: "Por papel de parede",
      },
      displays: {
        tag: "Monitores",
        title: "Papel de parede por monitor",
        description:
          "Clique com o botão direito em qualquer papel de parede e escolha Definir em uma tela específica. Cada monitor pode rodar o seu.",
        status: "Por monitor",
      },
      schedule: {
        tag: "Agenda",
        title: "Agendamento de dia e noite",
        description:
          "Dois papéis de parede, dois horários de troca, alternados automaticamente pelo daemon. Faixas de horário e troca solar pelo arquivo de configuração.",
        status: "Automático",
      },
      power: {
        tag: "Energia",
        title: "Consciente de energia",
        description:
          "Pausa na bateria e pausa automaticamente por monitor quando uma janela ali entra em tela cheia.",
        status: "Pausa automática",
      },
      newTab: {
        tag: "Nova aba",
        title: "Seu papel de parede em toda nova aba",
        description:
          "Uma extensão de navegador (Chrome, Brave, Edge, Firefox) espelha o papel de parede da área de trabalho, ou uma escolha só do navegador, na página de nova aba, através de uma ponte local que fala apenas com 127.0.0.1. Já está no repositório; a publicação nas lojas está pendente.",
        status: "Em breve",
      },
      themes: {
        tag: "Temas",
        title: "Temas e cores de destaque",
        description:
          "Claro, escuro ou seguindo o sistema, com seis paletas de destaque.",
        status: "6 paletas",
      },
    },
  },

  compare: {
    kicker: "Comparar",
    title: "Fresco frente aos papéis de parede do Linux.",
    lead: "O Fresco é o único app de papel de parede animado para Linux nesta tabela que combina interface gráfica, decodificação por hardware, suporte a X11 e Wayland e um catálogo embutido, de graça e com manutenção ativa. Aqui está a comparação completa com Hidamari, Komorebi, mpvpaper e Wallpaper Engine.",
    meter: (tools: number, caps: number) =>
      `Comparação · ${tools} ferramentas · ${caps} recursos`,
    thFeature: "Recurso",
    yes: "Sim",
    no: "Não",
    note: "O Wallpaper Engine é um produto pago e feito primeiro para Windows. O Komorebi não recebe mais manutenção.",
    detailLabel: "Comparar em detalhe:",
    vs: (tool: string) => `Fresco vs ${tool}`,
    rows: {
      gui: "App gráfico, sem terminal",
      x11: "Funciona no X11",
      wayland: "Funciona no Wayland (layer-shell)",
      hwDecode: "Decodificação por hardware, CPU baixa",
      cropRotate: "Recorte por arrasto e rotação",
      playlists: "Playlists",
      slideshow: "Apresentação de imagens",
      library: "Biblioteca de papéis de parede",
      catalog: "Catálogo embutido",
      perDisplay: "Papel de parede por monitor (interface)",
      schedules: "Agendamento de dia e noite",
      maintained: "Manutenção ativa",
      foss: "Gratuito e de código aberto",
    },
    cells: {
      partial: "Parcial",
      manual: "Manual",
      compositorOff: "Sem compositor",
      cropOnly: "Só recorte",
      workshop: "Workshop",
    },
  },

  whatsNew: {
    kicker: (version: string) => `Novidades · v${version}`,
    title: "Quatro widgets, pintados no papel de parede.",
    lead: (version: string) =>
      `O que chegou na v${version}. Sem janela extra, sem nada para clicar, idêntico no X11 e no layer-shell. Os quatro vêm desativados e, com música tocando e todos ligados, o custo medido foi de 0,8% de um núcleo de CPU. Cada item aqui está reproduzido no CHANGELOG no GitHub.`,
    changelog: "Changelog completo",
    patch: (n: string) => `Patch ${n}`,
    items: {
      lyrics: {
        title: "Letras sincronizadas",
        body: "A linha atual, em sincronia com o que estiver tocando via MPRIS. Primeiro arquivos .lrc locais, depois o LRCLIB. Quatro predefinições e ajuste de sincronia.",
      },
      clock: {
        title: "Relógio, seis temas",
        body: "Digital, Minimal, Segment, Stacked, Wordy e Card, um painel translúcido com mostrador analógico desenhado. 12 ou 24 horas, data opcional.",
      },
      visualizer: {
        title: "Visualizador de áudio",
        body: "Bars, Mirror, Wave, Dots ou Ring, com seletor de cor, mistura de duas cores ou arco-íris. Pede permissão antes de escutar o seu áudio.",
      },
      disc: {
        title: "Capa do álbum em um disco",
        body: "A capa da faixa atual em um disco girando. Ele para de girar no instante em que a reprodução pausa.",
      },
    },
  },

  howItWorks: {
    kicker: "Como funciona",
    title: "Três cliques e pode esquecer.",
    lead: "Abra o Fresco, clique em adicionar, clique em definir, feche. O daemon mantém o papel de parede rodando, mesmo depois de reiniciar.",
    step: (n: string) => `Passo ${n}`,
    steps: {
      pick: {
        title: "Escolha a mídia",
        description:
          "Abra o Fresco pelo menu de aplicativos e escolha um vídeo, GIF, imagem, pasta ou playlist.",
      },
      set: {
        title: "Clique em Definir",
        description:
          "Defina como papel de parede. A reprodução começa na sua área de trabalho na hora.",
      },
      close: {
        title: "Feche o app",
        description:
          "Feche a janela. Um daemon leve mantém o papel de parede rodando, mesmo depois de reiniciar.",
      },
    },
  },

  videos: {
    kicker: "Veja funcionando",
    title: "Menos de um minuto cada. Sem narração.",
    lead: "Gravações curtas de tela do Fresco em uma área de trabalho real. Nada é carregado do YouTube até você apertar play.",
    more: "Mais no YouTube",
    inDevelopment: "Em desenvolvimento",
    play: (title: string) => `Reproduzir: ${title}`,
    items: {
      "YWzD3-xkCEc": {
        tag: "Adicionar por link",
        blurb:
          "Copie um link do Pinterest, cole no Fresco e defina como papel de parede. Sem download, sem malabarismo com arquivos.",
      },
      C1MqrhGkovQ: {
        tag: "Widgets de letra",
        blurb:
          "Letras sincronizadas e um relógio desenhados em um papel de parede animado no Wayland e no X11. Lançados na v1.1.36, junto com o visualizador de áudio e o disco com a capa do álbum.",
      },
    },
  },

  supported: {
    kicker: "Ambientes testados",
    title: "Onde o Fresco roda.",
    lead: "Em qualquer área de trabalho X11, incluindo o DDE do deepin 25, e nos compositores Wayland com layer-shell (COSMIC, Hyprland, Sway e KDE Plasma 6), nas distribuições Debian e Ubuntu mais populares. O GNOME com Wayland recebe um quadro estático.",
    deployed: (distros: number, formats: number) =>
      `Testado: 6 compositores com animação · 1 fallback estático · ${distros} distros · ${formats} formatos`,
    sessionsTitle: "Sessões e compositores",
    distrosTitle: (n: number) => `Distribuições testadas · ${n}`,
    formatsTitle: (n: number) => `Formatos suportados · ${n}`,
    live: "Papel de parede animado",
    fallback: "Quadro estático",
    sessions: {
      x11: {
        label: "X11 (qualquer área de trabalho)",
        detail: "GNOME, KDE, XFCE, MATE, Cinnamon, Budgie",
      },
      deepin: {
        label: "Deepin 25 (DDE, X11)",
        detail:
          "Adaptação automática ao DDE, os ícones continuam visíveis. Verificado pela comunidade no deepin 25 Community build1. Também na loja de apps do deepin.",
      },
      wayland: {
        label: "Wayland layer-shell",
        detail: "COSMIC, Hyprland, Sway, KDE Plasma 6, wlroots",
      },
      gnome: {
        label: "GNOME no Wayland",
        detail: "Quadro estático (o Mutter não tem superfície animada)",
      },
    },
    fieldReport: "Relato de campo · deepin 25",
    verifiedEnv: "Ambiente verificado",
    testimonialRole: "Testador da comunidade deepin",
    envLabels: {
      session: "Sessão",
      os: "SO",
      gpu: "GPU",
    },
    footnote:
      "O deepin 25 usa X11 por padrão, e é nessa sessão que o Fresco foi verificado ali. O Treeland, o compositor Wayland do próprio deepin, ainda está em desenvolvimento, então o Fresco não faz nenhuma afirmação sobre o deepin no Wayland por enquanto.",
  },

  download: {
    kicker: "Baixar",
    title: "Instale no Debian, Ubuntu, Pop!_OS e Mint.",
    badge: "X11 · Wayland",
    lead: "O instalador oficial de uma linha ou o pacote .deb. Os dois caminhos vão para a sua área de transferência e rodam na hora. O Fresco continua reproduzindo depois que você fecha a janela.",
    cardTitle: "Instalação em uma linha",
    cardBody:
      "Execute isto em um terminal. Ele baixa e instala o .deb mais recente para você, sempre a versão mais nova:",
    terminalTitle: "fresco install",
    aptComment: "Já baixou o .deb?",
    releases: "Ver todas as versões",
    gpuNote:
      "Para o menor uso de CPU, instale o driver de decodificação por hardware da sua GPU (driver Intel media VA, drivers VA do Mesa ou o driver proprietário da NVIDIA para NVDEC).",
    storeLabel: "Também na loja de apps do deepin",
    storeBody:
      "No deepin 25, abra a loja de apps, pesquise por Fresco e clique em Instalar. As atualizações chegam pela loja.",
    copy: "Copiar",
    copied: "Copiado",
  },

  faq: {
    kicker: "Perguntas frequentes",
    title: "Perguntas, respondidas.",
    lead: "Tudo o que você precisa saber antes de definir o seu primeiro papel de parede animado no Linux.",
    items: [
      {
        q: "Existe um Wallpaper Engine para Linux?",
        a: "Existe. O Fresco é um app gratuito e de código aberto de papel de parede animado para Linux que funciona como o Wallpaper Engine: escolha um vídeo, GIF ou imagem e defina como plano de fundo animado. Ele é feito para uso gráfico e não precisa de Steam nem de Proton.",
      },
      {
        q: "Como coloco um vídeo como papel de parede no Ubuntu ou no Pop!_OS?",
        a: "Instale o .deb do Fresco, abra pelo menu de aplicativos, clique em Adicionar, escolha o vídeo, recorte ou gire se quiser e clique em Definir como papel de parede. Feche o app e o vídeo continua tocando como plano de fundo.",
      },
      {
        q: "Um papel de parede em vídeo consome CPU ou bateria?",
        a: "Não. O Fresco decodifica vídeo na GPU pelo mpv (VA-API e NVDEC), então o uso de CPU fica perto de zero e a memória fica em torno de 120 a 150 MB. Ele pode pausar automaticamente na bateria e pausa sozinho em qualquer monitor com uma janela em tela cheia.",
      },
      {
        q: "O Fresco funciona no Wayland e no COSMIC?",
        a: "Funciona. O Fresco roda papéis de parede animados em compositores Wayland com layer-shell através de um backend mpvpaper embutido e supervisionado: COSMIC (Pop!_OS 24.04), Hyprland, Sway, KDE Plasma 6 e outros compositores wlroots. Desde a v1.1.1 ele traz duas builds do mpvpaper e testa em tempo de execução, então funciona tanto em distribuições com libmpv1 quanto com libmpv2. No X11 ele funciona em qualquer área de trabalho.",
      },
      {
        q: "O Fresco funciona no GNOME?",
        a: "No GNOME com sessão X11, sim, papéis de parede animados completos. No GNOME com Wayland, o Mutter não expõe uma superfície de papel de parede animado, então o Fresco mostra um quadro estático do papel de parede escolhido em vez de fingir que anima.",
      },
      {
        q: "Um papel de parede em vídeo pode ter som?",
        a: "Pode. Cada papel de parede lembra o próprio estado de mudo e o volume, então você pode ativar o som de um vídeo específico e a escolha vale toda vez que ele for definido. Papéis de parede começam mudos por padrão.",
      },
      {
        q: "Dá para recortar ou girar um papel de parede?",
        a: "Dá. O editor tem um quadro de recorte por arrasto e rotação de 90 graus, então você pode escolher exatamente a região ou endireitar um vídeo de celular gravado deitado. Os dois são aplicados na GPU e lembrados por papel de parede.",
      },
      {
        q: "O papel de parede continua depois de reiniciar?",
        a: "Continua. O Fresco adiciona uma entrada de inicialização automática que restaura o papel de parede animado no login e se conserta sozinha se a entrada sumir. Você pode desligar isso nas configurações.",
      },
      {
        q: "Quais formatos de mídia são suportados?",
        a: "Vídeo em loop (mp4, webm, mkv, avi, mov), GIFs animados, imagens estáticas (jpg, png, webp), uma pasta de imagens como apresentação de slides com transições crossfade, fade, slide ou Ken Burns, e playlists com vários vídeos.",
      },
      {
        q: "Ele suporta vários monitores?",
        a: "Suporta. Você pode definir um papel de parede diferente em cada tela, e o Fresco pausa o papel de parede daquela saída quando uma janela ali entra em tela cheia. A troca de monitor a quente é imediata no X11; no Wayland uma tela recém-conectada é reconhecida na próxima aplicação (a detecção automática chega com o motor v1.0).",
      },
      {
        q: "Qual a diferença entre o Fresco e o Wallpaper Engine?",
        a: "O Wallpaper Engine é um produto pago, feito primeiro para Windows, que no Linux só roda através do Steam Play e do Proton. O Fresco é gratuito, de código aberto (GPL-3.0) e nativo do Linux: sem Steam, sem Proton, sem camada de compatibilidade. No lugar do Steam Workshop ele tem um catálogo embutido de papéis de parede selecionados e licenciados, e suporta X11 e compositores Wayland com layer-shell diretamente.",
      },
      {
        q: "Qual a diferença entre o Fresco e o Hidamari, o Komorebi e o mpvpaper?",
        a: "O Fresco é feito para uso gráfico, acelerado por hardware, e trata papéis de parede em vídeo, GIF, imagem, apresentação e playlist em um único app, tanto no X11 quanto no Wayland. Ele recebe manutenção ativa, diferente do Komorebi, e não exige linha de comando, diferente do mpvpaper.",
      },
      {
        q: "Onde encontro papéis de parede animados para Linux?",
        a: "Dentro do próprio Fresco. O catálogo embutido (menu, depois Procurar papéis de parede) traz papéis de parede em vídeo selecionados e devidamente licenciados que você define em dois cliques, com a licença e o autor visíveis em cada item. Você também pode colar a URL direta de um vídeo ou imagem, ou adicionar os seus próprios arquivos.",
      },
      {
        q: "O papel de parede pode mudar sozinho entre dia e noite?",
        a: "Pode. Abra o menu, escolha Avançado e depois Papel de parede de dia e noite: escolha dois papéis de parede e os horários de troca, e o daemon alterna automaticamente sem reiniciar. Faixas de horário arbitrárias e troca no nascer ou pôr do sol (com coordenadas manuais) estão disponíveis pelo config.toml.",
      },
      {
        q: "Como defino um papel de parede diferente em cada monitor?",
        a: "Clique com o botão direito em qualquer papel de parede da biblioteca e escolha Definir em uma tela específica. Cada monitor conectado aparece com a sua resolução. Escolher Mostrar o padrão em todas as telas limpa as escolhas por monitor.",
      },
      {
        q: "Dá para mostrar a letra da música na área de trabalho do Linux?",
        a: "Dá. O Fresco desenha letras sincronizadas no seu papel de parede, acompanhando o que estiver tocando no sistema via MPRIS: navegadores, apps de música, reprodutores de vídeo. São quatro predefinições, uma grade de nove posições, um controle de ajuste de sincronia, uma linha seguinte opcional e título e artista opcionais. As letras vêm primeiro de um arquivo .lrc local e depois do LRCLIB, uma base gratuita mantida pela comunidade. O Firefox é o reprodutor mais confiável; o cliente nativo do Spotify no Linux informa a posição de reprodução errada, mas o Spotify no navegador funciona bem.",
      },
      {
        q: "O Linux tem widgets de área de trabalho como o Conky?",
        a: "Tem, e o Fresco adiciona quatro deles que não precisam de painel, de extensão nem de suporte do seu ambiente: letras sincronizadas, um relógio com seis temas, um visualizador de áudio com cinco estilos e a capa da faixa atual em um disco girando. Eles são pintados no próprio papel de parede em vez de em uma janela, então nunca ficam sobre as suas janelas, nunca interceptam um clique e funcionam em ambientes que não têm camada de widgets própria, incluindo COSMIC, Hyprland e Sway. Os quatro vêm desativados. Diferente do Conky, ainda não há widgets de monitoramento do sistema, então nada de CPU, RAM ou rede. O GNOME com Wayland é o único lugar onde os widgets não rodam, porque não existe superfície de papel de parede animado para desenhar.",
      },
      {
        q: "Dá para ter um visualizador de música no plano de fundo?",
        a: "Dá. O visualizador de áudio do Fresco reage ao que o seu sistema estiver tocando, em um de cinco estilos (Bars, Mirror, Wave, Dots, Ring) com seletor de cor, mistura de duas cores ou arco-íris. Ele vem desativado e pede consentimento na primeira vez que você o liga, porque precisa escutar a sua saída de áudio. Com música tocando e os quatro widgets ligados, o custo medido foi de 0,8% de um núcleo de CPU, quase tudo por causa da captura de áudio, já que nada é redesenhado a menos que o conteúdo mude.",
      },
      {
        q: "O Fresco está na loja de apps do deepin?",
        a: "Está. No deepin 25, abra a loja de apps, pesquise por Fresco e clique em Instalar. Ele é publicado na loja de apps da comunidade do deepin, então as atualizações chegam pela loja. O .deb e o instalador de uma linha também funcionam.",
      },
      {
        q: "O Fresco é gratuito?",
        a: "É. O Fresco é totalmente gratuito e de código aberto sob a licença GPL-3.0. Não existe versão paga.",
      },
    ],
  },

  footer: {
    github: "GitHub",
    license: "Licença",
    tagline: "Rust + GTK4 + mpv",
    sound: "Alternar som",
  },

  featureList: [
    "Catálogo embutido de papéis de parede selecionados e licenciados",
    "Papéis de parede em vídeo, GIF, imagem, apresentação e playlist",
    "Widgets desenhados no papel de parede: letras sincronizadas, relógio, visualizador de áudio, capa do álbum",
    "Adicionar papéis de parede por URL direta",
    "Agendamento de papel de parede de dia e noite (mais faixas de horário e modo solar via configuração)",
    "Papel de parede por monitor pela interface gráfica",
    "Recuperação automática de áudio quando o servidor de som inicia tarde",
    "Socket de controle JSON programável",
    "Reprodução acelerada por hardware (VA-API, NVDEC)",
    "Funciona no X11 e em compositores Wayland com layer-shell",
    "Editor de recorte por arrasto e rotação de 90 graus",
    "Som e volume por papel de parede",
    "Transições de apresentação (crossfade, fade, slide, Ken Burns)",
    "Biblioteca de papéis de parede com busca",
    "Papel de parede diferente em cada monitor",
    "Pausa na bateria e pausa automática em tela cheia",
    "Restaura automaticamente no login",
    "Temas e cores de destaque",
  ],

  softwareDescription:
    "O Fresco é um app gratuito e de código aberto de papel de parede animado para Linux. Ele define papéis de parede em vídeo, GIF, imagem, apresentação e playlist como plano de fundo animado, com reprodução acelerada por hardware, e pode desenhar quatro widgets no próprio papel de parede: letras sincronizadas, um relógio, um visualizador de áudio e a capa do álbum em um disco girando. Uma alternativa gratuita ao Wallpaper Engine para Pop!_OS, Ubuntu, Linux Mint, Debian e elementary OS, no X11 e em compositores Wayland com layer-shell (COSMIC, Hyprland, Sway, KDE Plasma 6).",
};
