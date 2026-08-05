import type { Dictionary } from "./en";

/**
 * 简体中文。产品名与平台术语保持英文原文（Fresco、X11、Wayland、layer-shell、
 * mpv、GPL-3.0 等），这是中文 Linux 文档的通行写法。
 */
export const zhCn: Dictionary = {
  meta: {
    title: "Fresco - Linux 动态壁纸 | 免费的 Wallpaper Engine 替代方案",
    description:
      "面向 Linux 的免费开源动态壁纸应用。内置壁纸库，把视频或 GIF 设为桌面背景，每台显示器独立壁纸，昼夜自动切换。在 X11 与 Wayland 上均支持硬件加速。",
    ogTitle: "Fresco - Linux 动态壁纸",
    ogDescription:
      "内置壁纸库、每台显示器独立壁纸、昼夜定时切换，以及在 X11 与 Wayland 上几乎不占 CPU 的硬件加速播放。一款免费的 Wallpaper Engine 替代方案。",
    twitterDescription:
      "支持硬件加速的 Linux 动态壁纸，可在 X11 与 Wayland 上运行。免费开源的 Wallpaper Engine 替代方案。",
    ogImageAlt: "Fresco。Linux 上终于有了真正好用的壁纸。",
    keywords: [
      "linux 动态壁纸",
      "linux 视频壁纸",
      "ubuntu 动态壁纸",
      "wallpaper engine linux 替代",
    ],
  },

  nav: {
    home: "Fresco 首页",
    features: "功能",
    compare: "对比",
    whatsNew: "更新",
    download: "下载",
    cta: "获取 Fresco",
    star: "在 GitHub 上为 Fresco 点星",
    starWithCount: (n: string) => `在 GitHub 上为 Fresco 点星（${n} 颗星）`,
  },

  language: {
    label: "语言",
    change: "切换语言",
  },

  theme: {
    toggle: "切换主题",
    light: "浅色",
    dark: "深色",
  },

  hero: {
    titleLead: "Linux 上终于有了",
    /** Separator before the accented tail; empty where CJK needs none. */
    titleGap: "",
    titleEm: "真正好用的壁纸。",
    body: "把任意视频、GIF 或图片设为 Linux 桌面背景。硬件加速播放让 CPU 占用几乎为零，X11 与 Wayland 都可用。关掉应用，守护进程会继续播放。",
    install: "安装 Fresco",
    star: "在 GitHub 点星",
  },

  stats: {
    ariaLabel: "项目数据",
    downloads: "累计下载量",
    downloadsUnknown: "github 下载量",
    stars: "github 星标",
    version: "最新版本",
    license: "免费开源",
  },

  glance: {
    ariaLabel: "Fresco 概览",
    caption: "fresco 概览",
    labelWhat: "这是什么",
    labelPlatforms: "支持平台",
    labelWidgets: "桌面组件",
    labelLicense: "许可协议",
    labelInstall: "安装",
    what: "Fresco 是一款面向 Linux 的免费开源动态壁纸应用：可以把视频、GIF、图片、幻灯片和播放列表设为动态桌面背景，并使用 GPU 硬件解码。它既是免费的 Wallpaper Engine 替代方案，也是 Wayland 上 mpvpaper 的图形界面。",
    platforms:
      "任何 X11 桌面（Ubuntu、Pop!_OS、Linux Mint、Debian），以及支持 layer-shell 的 Wayland 合成器：COSMIC、Hyprland、Sway、KDE Plasma 6。GNOME Wayland 下会回退为静态画面。",
    widgets:
      "四个直接绘制在壁纸上而非窗口里的组件：时间同步歌词、六种主题的时钟、音频可视化，以及旋转唱片上的专辑封面。它们不会浮在窗口上方，也不会拦截点击。四个默认都关闭。GNOME Wayland 没有动态壁纸绘制面，因此无法使用。",
    licenseLead: "GPL-3.0，永久免费。",
    licenseLink: "GitHub 源码",
    licenseTail: "使用 Rust、GTK4 和 mpv 构建。",
  },

  features: {
    kicker: "功能",
    title: "任意媒体，任意显示器，CPU 毫无压力。",
    lead: "Fresco 可在 X11 与 Wayland 上设置视频、GIF、图片、幻灯片和播放列表壁纸，解码全部交给 GPU，因此动态壁纸的开销与静态壁纸相差无几。完整规格如下：",
    manifest: (n: number) => `功能清单：${n} 项`,
    /** Sentence-final mark after each row title. */
    titleSuffix: "。",
    thCapability: "能力",
    thWhatYouGet: "具体内容",
    thStatus: "状态",
    footnote:
      "gnome wayland：回退为静态画面（mutter 未提供动态绘制面），桌面组件同样需要这个绘制面，因此在该环境下不可用。上表其余功能均可正常使用。",
    tally: (shipping: number, total: number, soon: number) =>
      `${total} 项中已发布 ${shipping} 项 · ${soon} 项预览中 · 0 项已废弃`,
    rows: {
      hwDecode: {
        tag: "硬件解码",
        title: "硬件加速播放",
        description:
          "解码通过 mpv 在 GPU 上完成（VA-API 或 NVDEC）。一段 4K 视频壁纸的 CPU 开销与静态图片相当。",
        status: "cpu 近乎为零",
      },
      sessions: {
        tag: "会话",
        title: "X11 与 Wayland",
        description:
          "在任意 X11 桌面上使用桌面窗口后端，并为 COSMIC、Hyprland、Sway 和 KDE Plasma 6 提供 layer-shell 后端。GNOME Wayland 会回退为静态画面。",
        status: "x11 · layer-shell",
      },
      catalog: {
        tag: "壁纸库",
        title: "内置壁纸库",
        description:
          "在应用内浏览精选且授权明确的壁纸（菜单 →“浏览壁纸”），两次点击即可设置。也可以直接粘贴链接。",
        status: "应用内",
      },
      video: {
        tag: "视频 · gif",
        title: "视频与 GIF 壁纸",
        description: "把任意 mp4、webm、mkv 或动态 GIF 循环播放为桌面背景。",
        status: "mp4 webm mkv gif",
      },
      slideshow: {
        tag: "幻灯片",
        title: "带转场的幻灯片",
        description: "用交叉淡化、淡入淡出或 Ken Burns 效果轮播一个图片文件夹。",
        status: "4 种转场",
      },
      playlist: {
        tag: "播放列表",
        title: "视频播放列表",
        description: "把多段视频排入队列，让 Fresco 依次循环播放。",
        status: "自动轮播",
      },
      lyrics: {
        tag: "歌词 · 时钟",
        title: "歌词与时钟组件",
        description:
          "跟随 MPRIS 上正在播放内容的时间同步歌词（优先本地 .lrc，其次 LRCLIB），以及六种主题可选的时钟。直接绘制在壁纸上，不会浮在窗口之上。默认关闭。",
        status: "默认关闭",
      },
      visualiser: {
        tag: "可视化",
        title: "音频可视化与专辑封面",
        description:
          "五种可视化风格（Bars、Mirror、Wave、Dots、Ring），支持取色器、双色渐变或彩虹配色，还可把当前曲目的封面放在旋转唱片上。开启监听音频前会先征求同意。",
        status: "单核的 0.8%",
      },
      editor: {
        tag: "编辑器",
        title: "裁剪与旋转",
        description:
          "拖动画框选择区域，旋转 90 度校正横拍的片段。两者都在 GPU 上零拷贝完成。",
        status: "零拷贝",
      },
      audio: {
        tag: "音频",
        title: "逐张壁纸的声音设置",
        description:
          "为某段视频取消静音并调整音量，Fresco 会记住这张壁纸的设置。",
        status: "逐张记忆",
      },
      displays: {
        tag: "显示器",
        title: "每台显示器独立壁纸",
        description:
          "右键点击任意壁纸，选择“设置到指定显示器”。每台显示器都可以有自己的壁纸。",
        status: "分屏设置",
      },
      schedule: {
        tag: "定时",
        title: "昼夜定时切换",
        description:
          "两张壁纸、两个切换时间，由守护进程自动更换。任意时间段与日出日落切换可通过配置文件设置。",
        status: "全自动",
      },
      power: {
        tag: "电源",
        title: "电源感知",
        description:
          "使用电池时自动暂停；某台显示器上有窗口全屏时，也会单独暂停该显示器。",
        status: "自动暂停",
      },
      newTab: {
        tag: "浏览器新标签页",
        title: "每个新标签页都是你的壁纸",
        description:
          "配套浏览器扩展（Chrome、Brave、Edge、Firefox）会通过仅与 127.0.0.1 通信的本地桥接，把桌面壁纸或浏览器专属的一张壁纸显示在新标签页上。目前已在仓库中提供，商店上架待定。",
        status: "即将推出",
      },
      themes: {
        tag: "主题",
        title: "主题与强调色",
        description: "浅色、深色或跟随系统，并提供六种强调色方案。",
        status: "6 种配色",
      },
    },
  },

  compare: {
    kicker: "对比",
    title: "Fresco 与 Linux 壁纸工具横评。",
    lead: "在这张表里，Fresco 是唯一一款同时具备图形界面、硬件解码、X11 与 Wayland 支持和内置壁纸库，并且免费、仍在积极维护的 Linux 动态壁纸应用。以下是与 Hidamari、Komorebi、mpvpaper 和 Wallpaper Engine 的完整对比。",
    meter: (tools: number, caps: number) =>
      `对比 · ${tools} 款工具 · ${caps} 项能力`,
    thFeature: "功能",
    yes: "支持",
    no: "不支持",
    note: "Wallpaper Engine 是一款以 Windows 为主的付费产品。Komorebi 已停止维护。",
    detailLabel: "查看详细对比：",
    vs: (tool: string) => `Fresco 对比 ${tool}`,
    rows: {
      gui: "图形界面，无需终端",
      x11: "支持 X11",
      wayland: "支持 Wayland（layer-shell）",
      hwDecode: "硬件解码，低 CPU 占用",
      cropRotate: "拖动裁剪与旋转",
      playlists: "播放列表",
      slideshow: "图片幻灯片",
      library: "壁纸库管理",
      catalog: "内置壁纸目录",
      perDisplay: "每台显示器独立壁纸（图形界面）",
      schedules: "昼夜定时切换",
      maintained: "积极维护中",
      foss: "免费开源",
    },
    cells: {
      partial: "部分支持",
      manual: "需手动",
      compositorOff: "需关闭合成",
      cropOnly: "仅裁剪",
      workshop: "创意工坊",
    },
  },

  whatsNew: {
    kicker: (version: string) => `更新 · v${version}`,
    title: "四个桌面组件，直接画进壁纸里。",
    lead: (version: string) =>
      `v${version} 带来的内容。没有额外窗口，不会挡住点击，在 X11 与 layer-shell 上表现一致。四个组件默认全部关闭；在播放音乐并全部开启的情况下，实测开销为单个 CPU 核心的 0.8%。此处每一条都同步记录在 GitHub 的 CHANGELOG 中。`,
    changelog: "完整更新日志",
    patch: (n: string) => `更新 ${n}`,
    items: {
      lyrics: {
        title: "同步歌词",
        body: "当前这一行，与 MPRIS 上正在播放的内容保持同步。优先读取本地 .lrc 文件，其次使用 LRCLIB。提供四种预设和同步偏移。",
      },
      clock: {
        title: "时钟，六种主题",
        body: "Digital、Minimal、Segment、Stacked、Wordy，以及带手绘指针表盘的半透明面板 Card。支持 12/24 小时制，日期可选。",
      },
      visualizer: {
        title: "音频可视化",
        body: "Bars、Mirror、Wave、Dots 或 Ring，支持取色器、双色渐变或彩虹配色。监听音频前会先征求同意。",
      },
      disc: {
        title: "唱片上的专辑封面",
        body: "当前曲目的封面放在旋转的唱片上。播放一暂停，唱片立刻停转。",
      },
    },
  },

  howItWorks: {
    kicker: "使用方法",
    title: "三次点击，然后就可以忘了它。",
    lead: "打开 Fresco，点添加，点设置，关闭。守护进程会让壁纸一直运行，重启后也不例外。",
    step: (n: string) => `步骤 ${n}`,
    steps: {
      pick: {
        title: "选择媒体",
        description:
          "从应用菜单打开 Fresco，选择一段视频、GIF、图片、文件夹或播放列表。",
      },
      set: {
        title: "点击设置",
        description: "把它设为壁纸，桌面上立刻开始播放。",
      },
      close: {
        title: "关闭应用",
        description:
          "关掉窗口。一个轻量守护进程会让壁纸持续运行，重启之后也会恢复。",
      },
    },
  },

  videos: {
    kicker: "实际效果",
    title: "每段不到一分钟，无需解说。",
    lead: "在真实桌面上录制的 Fresco 短片。在你按下播放之前，不会从 YouTube 加载任何内容。",
    more: "在 YouTube 上查看更多",
    inDevelopment: "开发中",
    play: (title: string) => `播放：${title}`,
    items: {
      "YWzD3-xkCEc": {
        tag: "从链接添加",
        blurb:
          "复制一个 Pinterest 链接，粘贴进 Fresco，直接设为壁纸。无需下载，也不用管理文件。",
      },
      C1MqrhGkovQ: {
        tag: "歌词组件",
        blurb:
          "在 Wayland 与 X11 的动态壁纸上绘制同步歌词和时钟。随 v1.1.36 发布，同期还有音频可视化和专辑封面唱片。",
      },
    },
  },

  supported: {
    kicker: "已验证环境",
    title: "Fresco 能在哪里运行。",
    lead: "支持任意 X11 桌面（包括 Deepin 25 的 DDE），以及支持 layer-shell 的 Wayland 合成器（COSMIC、Hyprland、Sway 和 KDE Plasma 6），覆盖主流的 Debian 与 Ubuntu 发行版。GNOME Wayland 下会回退为静态画面。",
    deployed: (distros: number, formats: number) =>
      `已验证：6 个动态合成器 · 1 个静态回退 · ${distros} 个发行版 · ${formats} 种格式`,
    sessionsTitle: "会话与合成器",
    distrosTitle: (n: number) => `已测试发行版 · ${n}`,
    formatsTitle: (n: number) => `支持格式 · ${n}`,
    live: "动态壁纸",
    fallback: "静态回退",
    sessions: {
      x11: {
        label: "X11（任意桌面）",
        detail: "GNOME、KDE、XFCE、MATE、Cinnamon、Budgie",
      },
      deepin: {
        label: "Deepin 25（DDE，X11）",
        detail:
          "自动适配 DDE，桌面图标保持可见。已由社区在 Deepin 25 社区版 build1 上验证。",
      },
      wayland: {
        label: "Wayland layer-shell",
        detail: "COSMIC、Hyprland、Sway、KDE Plasma 6、wlroots",
      },
      gnome: {
        label: "GNOME on Wayland",
        detail: "回退为静态画面（Mutter 没有动态绘制面）",
      },
    },
    fieldReport: "实地反馈 · deepin 25",
    verifiedEnv: "验证环境",
    testimonialRole: "Deepin 社区测试者",
    envLabels: {
      session: "会话",
      os: "系统",
      gpu: "显卡",
    },
    footnote:
      "deepin 25 默认使用 x11，fresco 在该系统上也正是在这个会话下通过验证的。deepin 自研的 wayland 合成器 treeland 仍在开发中，因此 fresco 目前不对 deepin 的 wayland 环境作任何承诺。",
  },

  download: {
    kicker: "下载",
    title: "可部署在 Debian、Ubuntu、Pop!_OS 与 Mint 上。",
    badge: "x11 · wayland",
    lead: "官方一行安装命令，或直接下载 .deb 安装包。两种方式都能一键复制到剪贴板并立即执行。关闭窗口后 Fresco 仍会继续播放。",
    cardTitle: "一行命令安装",
    cardBody:
      "在终端中运行以下命令。它会自动下载并安装最新的 .deb，始终是最新版本：",
    terminalTitle: "fresco install",
    aptComment: "已经下载好 .deb 了？",
    releases: "查看全部版本",
    gpuNote:
      "为了把 CPU 占用降到最低，请安装显卡对应的硬件解码驱动（Intel media VA 驱动、Mesa VA 驱动，或用于 NVDEC 的 NVIDIA 专有驱动）。",
    copy: "复制",
    copied: "已复制",
  },

  faq: {
    kicker: "常见问题",
    title: "常见疑问，一一解答。",
    lead: "在 Linux 上设置第一张动态壁纸之前，你需要知道的一切。",
    items: [
      {
        q: "Linux 上有 Wallpaper Engine 吗？",
        a: "有。Fresco 是一款面向 Linux 的免费开源动态壁纸应用，用法和 Wallpaper Engine 一样：选一段视频、GIF 或图片，把它设为动态桌面背景。它以图形界面为主，不需要 Steam，也不需要 Proton。",
      },
      {
        q: "在 Ubuntu 或 Pop!_OS 上怎么把视频设成壁纸？",
        a: "安装 Fresco 的 .deb，从应用菜单打开，点击“添加”，选择视频，需要的话裁剪或旋转，然后点击“设为壁纸”。关闭应用后，视频会继续作为桌面背景播放。",
      },
      {
        q: "视频壁纸会不会耗 CPU 或电池？",
        a: "不会。Fresco 通过 mpv 在 GPU 上解码视频（VA-API 与 NVDEC），因此 CPU 占用接近于零，内存约为 120 到 150 MB。使用电池时可以自动暂停，某台显示器上有窗口全屏时，也会自动暂停该显示器的壁纸。",
      },
      {
        q: "Fresco 支持 Wayland 和 COSMIC 桌面吗？",
        a: "支持。Fresco 通过内置并受监控的 mpvpaper 后端，在支持 layer-shell 的 Wayland 合成器上运行动态壁纸：COSMIC（Pop!_OS 24.04）、Hyprland、Sway、KDE Plasma 6 以及其他 wlroots 合成器。自 v1.1.1 起，它会同时打包两个 mpvpaper 构建并在运行时探测，因此在 libmpv1 和 libmpv2 的发行版上都能工作。在 X11 上，它适用于任何桌面环境。",
      },
      {
        q: "Fresco 支持 GNOME 吗？",
        a: "在 GNOME 的 X11 会话下完全支持动态壁纸。在 GNOME 的 Wayland 会话下，Mutter 不提供动态壁纸绘制面，因此 Fresco 会显示所选壁纸的一帧静态画面，而不是假装在播放动画。",
      },
      {
        q: "视频壁纸可以播放声音吗？",
        a: "可以。每张壁纸都会记住自己的静音状态和音量，所以你可以只给某一段视频取消静音，之后每次设置它都会保持这个选择。壁纸默认以静音方式开始播放。",
      },
      {
        q: "可以裁剪或旋转壁纸吗？",
        a: "可以。编辑器提供拖动裁剪框和 90 度旋转，你可以精确选定区域，或者把横拍的手机视频转正。两者都在 GPU 上完成，并按壁纸分别记忆。",
      },
      {
        q: "重启之后壁纸还在吗？",
        a: "在。Fresco 会添加一个自启动项，在登录时自动恢复你的动态壁纸；如果该项丢失，它会自我修复。你可以在设置中关闭这项功能。",
      },
      {
        q: "支持哪些媒体格式？",
        a: "循环播放的视频（mp4、webm、mkv、avi、mov）、动态 GIF、静态图片（jpg、png、webp）、把图片文件夹作为幻灯片播放（支持交叉淡化、淡入淡出、滑动和 Ken Burns 转场），以及多视频播放列表。",
      },
      {
        q: "支持多显示器吗？",
        a: "支持。你可以为每台显示器设置不同的壁纸；某台显示器上的窗口全屏时，Fresco 只会暂停该输出的壁纸。显示器热插拔在 X11 上即时生效；在 Wayland 上，新接入的显示器会在下次应用时被识别（自动热插拔将随 v1.0 引擎推出）。",
      },
      {
        q: "Fresco 和 Wallpaper Engine 有什么不同？",
        a: "Wallpaper Engine 是一款以 Windows 为主的付费产品，在 Linux 上只能通过 Steam Play 和 Proton 运行。Fresco 免费、开源（GPL-3.0），并且原生面向 Linux：不需要 Steam、不需要 Proton、不需要兼容层。它没有创意工坊，取而代之的是内置的精选授权壁纸库，并直接支持 X11 与 Wayland layer-shell 合成器。",
      },
      {
        q: "Fresco 和 Hidamari、Komorebi、mpvpaper 有什么不同？",
        a: "Fresco 以图形界面为主，支持硬件加速，在一个应用里同时处理视频、GIF、图片、幻灯片和播放列表壁纸，X11 与 Wayland 都可用。与 Komorebi 不同，它仍在积极维护；与 mpvpaper 不同，它完全不需要命令行。",
      },
      {
        q: "去哪里找 Linux 用的动态壁纸？",
        a: "就在 Fresco 里。内置壁纸库（菜单 →“浏览壁纸”）提供精选且授权明确的视频壁纸，两次点击即可设置，每一项都会显示许可协议和作者。你也可以粘贴视频或图片的直链，或者添加自己的文件。",
      },
      {
        q: "壁纸能在白天和夜晚之间自动切换吗？",
        a: "可以。打开菜单，选择“高级”，再选“昼夜壁纸”：挑两张壁纸并设定切换时间，守护进程会自动更换，无需重启。任意时间段以及日出日落切换（需手动填写坐标）可通过 config.toml 设置。",
      },
      {
        q: "怎么给每台显示器设置不同的壁纸？",
        a: "在壁纸库中右键点击任意壁纸，选择“设置到指定显示器”。每台已连接的显示器都会连同分辨率一起列出。选择“在所有显示器上显示默认壁纸”即可清除按显示器的单独设置。",
      },
      {
        q: "可以在 Linux 桌面上显示歌词吗？",
        a: "可以。Fresco 会把时间同步的歌词绘制到壁纸上，跟随系统中通过 MPRIS 播放的任何内容：浏览器、音乐应用、视频播放器。提供四种预设、九宫格位置、同步偏移滑块、可选的下一行，以及可选的曲名与歌手。歌词优先来自本地 .lrc 文件，其次来自社区运营的免费数据库 LRCLIB。Firefox 是最可靠的播放器；Spotify 的 Linux 原生客户端上报的播放进度有问题，但浏览器版 Spotify 没有这个毛病。",
      },
      {
        q: "Linux 有类似 Conky 的桌面组件吗？",
        a: "有，而且 Fresco 新增的这四个既不需要面板，也不需要扩展，更不需要桌面环境本身支持：同步歌词、六种主题的时钟、五种风格的音频可视化，以及旋转唱片上的当前曲目封面。它们绘制在壁纸本身而不是窗口里，因此永远不会盖在窗口上方，也不会拦截点击，并且可以在没有自带组件层的桌面上运行，包括 COSMIC、Hyprland 和 Sway。四个默认全部关闭。与 Conky 不同的是，目前还没有系统监控类组件，所以没有 CPU、内存或网络读数。唯一无法运行组件的环境是 GNOME on Wayland，因为那里没有可供绘制的动态壁纸面。",
      },
      {
        q: "可以在桌面背景上做音乐可视化吗？",
        a: "可以。Fresco 的音频可视化会随系统正在播放的声音起伏，提供五种风格（Bars、Mirror、Wave、Dots、Ring），并支持取色器、双色渐变或彩虹配色。它默认关闭，首次启用时会征求你的同意，因为它需要监听音频输出。在播放音乐且四个组件全部开启的情况下，实测开销为单个 CPU 核心的 0.8%，其中绝大部分来自音频采集，因为内容没有变化时不会重绘。",
      },
      {
        q: "Fresco 免费吗？",
        a: "免费。Fresco 基于 GPL-3.0 协议完全免费且开源，没有任何付费版本。",
      },
    ],
  },

  footer: {
    github: "GitHub",
    license: "许可协议",
    tagline: "rust + gtk4 + mpv",
    sound: "开关音效",
  },

  featureList: [
    "内置精选授权壁纸库",
    "视频、GIF、图片、幻灯片与播放列表壁纸",
    "绘制在壁纸上的桌面组件：同步歌词、时钟、音频可视化、专辑封面",
    "通过直链添加壁纸",
    "昼夜壁纸定时切换（配置文件还支持时间段与日出日落）",
    "在图形界面中为每台显示器单独设置壁纸",
    "音频服务启动较晚时自动恢复声音",
    "可脚本化的 JSON 控制套接字",
    "硬件加速播放（VA-API、NVDEC）",
    "支持 X11 与 Wayland layer-shell 合成器",
    "拖动裁剪与 90 度旋转编辑器",
    "逐张壁纸的声音与音量设置",
    "幻灯片转场（交叉淡化、淡入淡出、滑动、Ken Burns）",
    "带搜索的壁纸库",
    "每台显示器可用不同壁纸",
    "电池供电时暂停，全屏时自动暂停",
    "登录时自动恢复",
    "主题与强调色",
  ],

  softwareDescription:
    "Fresco 是一款面向 Linux 的免费开源动态壁纸应用。它可以把视频、GIF、图片、幻灯片和播放列表设为动态桌面背景，支持硬件加速播放，还能把四个桌面组件直接绘制到壁纸上：时间同步歌词、时钟、音频可视化，以及旋转唱片上的专辑封面。它是面向 Pop!_OS、Ubuntu、Linux Mint、Debian 和 elementary OS 的免费 Wallpaper Engine 替代方案，支持 X11 以及 Wayland layer-shell 合成器（COSMIC、Hyprland、Sway、KDE Plasma 6）。",
};
