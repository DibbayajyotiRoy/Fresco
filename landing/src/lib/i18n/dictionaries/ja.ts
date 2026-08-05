import type { Dictionary } from "./en";

/**
 * 日本語. Product and platform nouns stay in Latin script (Fresco, X11,
 * Wayland, layer-shell, mpv, GPL-3.0 ...), which is how Japanese Linux
 * documentation writes them. Polite form throughout.
 */
export const ja: Dictionary = {
  meta: {
    title:
      "Fresco - Linux のライブ壁紙 | 無料の Wallpaper Engine 代替アプリ",
    description:
      "Linux 向けの無料オープンソースのライブ壁紙アプリ。内蔵カタログから壁紙を選ぶ、動画や GIF をデスクトップに設定する、モニターごとの壁紙、昼夜の自動切り替え。X11 と Wayland でハードウェア アクセラレーション対応。",
    ogTitle: "Fresco - Linux のライブ壁紙",
    ogDescription:
      "内蔵の壁紙カタログ、モニターごとの壁紙、昼夜スケジュール、そして X11 と Wayland での CPU ほぼゼロのハードウェア再生。無料の Wallpaper Engine 代替アプリです。",
    twitterDescription:
      "X11 と Wayland で動く、ハードウェア アクセラレーション対応の Linux ライブ壁紙。無料でオープンソースの Wallpaper Engine 代替アプリです。",
    ogImageAlt: "Fresco。ついに、ちゃんと動く Linux の壁紙。",
    keywords: [
      "linux ライブ壁紙",
      "linux 動画 壁紙",
      "ubuntu 動く壁紙",
      "wallpaper engine linux 代替",
    ],
  },

  nav: {
    home: "Fresco ホーム",
    features: "機能",
    compare: "比較",
    whatsNew: "新着",
    download: "ダウンロード",
    cta: "Fresco を入手",
    star: "GitHub で Fresco にスターを付ける",
    starWithCount: (n: string) =>
      `GitHub で Fresco にスターを付ける（スター ${n} 件）`,
  },

  language: {
    label: "言語",
    change: "言語を変更",
  },

  theme: {
    toggle: "テーマを切り替え",
    light: "ライト",
    dark: "ダーク",
  },

  hero: {
    titleLead: "ついに、ちゃんと動く",
    /** Separator before the accented tail; empty where CJK needs none. */
    titleGap: " ",
    titleEm: "Linux の壁紙。",
    body: "動画・GIF・画像を、そのまま Linux のデスクトップに設定できます。ハードウェア再生なので CPU 使用率はほぼゼロ、X11 でも Wayland でも動作します。アプリを閉じてもデーモンが再生を続けます。",
    install: "Fresco をインストール",
    star: "GitHub でスター",
  },

  stats: {
    ariaLabel: "プロジェクトの統計",
    downloads: "累計ダウンロード数",
    downloadsUnknown: "github のダウンロード数",
    stars: "github スター",
    version: "最新リリース",
    license: "無料・オープンソース",
  },

  glance: {
    ariaLabel: "Fresco の概要",
    caption: "fresco の概要",
    labelWhat: "概要",
    labelPlatforms: "対応環境",
    labelWidgets: "ウィジェット",
    labelLicense: "ライセンス",
    labelInstall: "インストール",
    what: "Fresco は Linux 向けの無料オープンソースのライブ壁紙アプリです。動画・GIF・画像・スライドショー・プレイリストを、GPU ハードウェア デコードで動くデスクトップ背景として設定できます。無料の Wallpaper Engine 代替であり、Wayland では mpvpaper の GUI としても使えます。",
    platforms:
      "あらゆる X11 デスクトップ（Ubuntu、Pop!_OS、Linux Mint、Debian）に加えて、Wayland の layer-shell コンポジタ（COSMIC、Hyprland、Sway、KDE Plasma 6）に対応。GNOME Wayland では静止フレームにフォールバックします。",
    widgets:
      "ウィンドウではなく壁紙そのものに描画される 4 つのウィジェット。時間同期の歌詞、6 テーマの時計、オーディオ ビジュアライザー、回転するレコード上のアルバム アート。ウィンドウの上に浮かぶことも、クリックを奪うこともありません。すべて既定でオフ。ライブ壁紙のサーフェスがない GNOME Wayland では利用できません。",
    licenseLead: "GPL-3.0、ずっと無料。",
    licenseLink: "GitHub のソース",
    licenseTail: "Rust、GTK4、mpv で作られています。",
  },

  features: {
    kicker: "機能",
    title: "どんなメディアも。どのモニターでも。CPU は静かなまま。",
    lead: "Fresco は動画・GIF・画像・スライドショー・プレイリストを X11 と Wayland の壁紙に設定します。デコードは GPU が担当するため、ライブ壁紙のコストは静止画とほぼ同じです。仕様の全体像はこちら:",
    manifest: (n: number) => `マニフェスト: ${n} 機能`,
    /** Sentence-final mark after each row title. */
    titleSuffix: "。",
    thCapability: "項目",
    thWhatYouGet: "内容",
    thStatus: "ステータス",
    footnote:
      "gnome wayland: 静止フレームへのフォールバック（mutter がライブ サーフェスを公開しないため）。ウィジェットも同じサーフェスを必要とするので、ここでは利用できません。上記のそれ以外はすべて動作します。",
    tally: (shipping: number, total: number, soon: number) =>
      `${total} 件中 ${shipping} 件がリリース済み · ${soon} 件がプレビュー · 廃止 0 件`,
    rows: {
      hwDecode: {
        tag: "hw デコード",
        title: "ハードウェア アクセラレーション再生",
        description:
          "デコードは mpv 経由で GPU 上（VA-API または NVDEC）。4K の動画壁紙でも、CPU 負荷は静止画とほぼ変わりません。",
        status: "cpu ほぼゼロ",
      },
      sessions: {
        tag: "セッション",
        title: "X11 と Wayland",
        description:
          "あらゆる X11 デスクトップ向けのデスクトップ ウィンドウ バックエンドに加えて、COSMIC・Hyprland・Sway・KDE Plasma 6 向けの layer-shell バックエンド。GNOME Wayland は静止フレームにフォールバックします。",
        status: "x11 · layer-shell",
      },
      catalog: {
        tag: "カタログ",
        title: "内蔵の壁紙カタログ",
        description:
          "厳選されたライセンス済みの壁紙をアプリ内（メニュー →「壁紙を閲覧」）で探して、2 クリックで設定。直接リンクの貼り付けにも対応します。",
        status: "アプリ内",
      },
      video: {
        tag: "動画 · gif",
        title: "動画と GIF の壁紙",
        description:
          "mp4、webm、mkv、アニメーション GIF をループ再生してデスクトップに。",
        status: "mp4 webm mkv gif",
      },
      slideshow: {
        tag: "スライドショー",
        title: "トランジション付きスライドショー",
        description:
          "フォルダー内の画像を、クロスフェード・フェード・Ken Burns で切り替え。",
        status: "4 種のトランジション",
      },
      playlist: {
        tag: "プレイリスト",
        title: "動画プレイリスト",
        description: "複数のクリップを並べて、Fresco に順番に再生させます。",
        status: "自動巡回",
      },
      lyrics: {
        tag: "歌詞 · 時計",
        title: "歌詞と時計のウィジェット",
        description:
          "MPRIS で再生中の曲に時間同期する歌詞（まずローカルの .lrc、次に LRCLIB）と、6 テーマから選べる時計。壁紙に描画されるので、ウィンドウの上に浮きません。既定でオフ。",
        status: "既定でオフ",
      },
      visualiser: {
        tag: "ビジュアライザー",
        title: "オーディオ ビジュアライザーとアルバム アート",
        description:
          "5 つのスタイル（Bars、Mirror、Wave、Dots、Ring）にカラー ピッカー、2 色ブレンド、レインボー。さらに再生中の曲のジャケットが回転するレコードに。ビジュアライザーは音声を聴く前に必ず確認します。",
        status: "1 コアの 0.8%",
      },
      editor: {
        tag: "エディター",
        title: "切り抜きと回転",
        description:
          "枠をドラッグして範囲を指定、90 度回転で横向きのクリップを直せます。どちらも GPU 上でゼロコピーのまま。",
        status: "ゼロコピー",
      },
      audio: {
        tag: "オーディオ",
        title: "壁紙ごとの音声",
        description:
          "動画のミュートを解除して音量を設定。Fresco はその壁紙の設定を記憶します。",
        status: "壁紙ごと",
      },
      displays: {
        tag: "ディスプレイ",
        title: "ディスプレイごとの壁紙",
        description:
          "壁紙を右クリックして「特定のディスプレイに設定」。モニターごとに別の壁紙を再生できます。",
        status: "モニターごと",
      },
      schedule: {
        tag: "スケジュール",
        title: "昼と夜のスケジュール",
        description:
          "2 つの壁紙と 2 つの切り替え時刻を決めれば、デーモンが自動で入れ替えます。任意の時間帯や日の出・日の入り連動は設定ファイルから。",
        status: "自動",
      },
      power: {
        tag: "電力",
        title: "電力を意識した設計",
        description:
          "バッテリー駆動中は一時停止。全画面ウィンドウがあるモニターでも自動的に停止します。",
        status: "自動停止",
      },
      newTab: {
        tag: "ブラウザー新規タブ",
        title: "新しいタブにも同じ壁紙を",
        description:
          "対応ブラウザー拡張（Chrome、Brave、Edge、Firefox）が、デスクトップの壁紙またはブラウザー専用の 1 枚を新規タブに表示します。通信は 127.0.0.1 のローカル ブリッジのみ。現在はリポジトリで公開中、ストア掲載は準備中です。",
        status: "近日公開",
      },
      themes: {
        tag: "テーマ",
        title: "テーマとアクセント",
        description:
          "ライト、ダーク、システム連動。6 種類のアクセント パレット付き。",
        status: "6 パレット",
      },
    },
  },

  compare: {
    kicker: "比較",
    title: "Fresco と Linux 壁紙アプリの現状。",
    lead: "この表の中で、GUI・ハードウェア デコード・X11 と Wayland の両対応・内蔵カタログをすべて備え、なおかつ無料で活発に開発が続いている Linux ライブ壁紙アプリは Fresco だけです。Hidamari、Komorebi、mpvpaper、Wallpaper Engine との全項目比較はこちら。",
    meter: (tools: number, caps: number) =>
      `比較 · ${tools} ツール · ${caps} 項目`,
    thFeature: "項目",
    yes: "対応",
    no: "非対応",
    note: "Wallpaper Engine は有料の Windows 優先製品です。Komorebi はすでにメンテナンスされていません。",
    detailLabel: "詳しく比較:",
    vs: (tool: string) => `Fresco と ${tool}`,
    rows: {
      gui: "GUI アプリ、ターミナル不要",
      x11: "X11 で動作",
      wayland: "Wayland（layer-shell）で動作",
      hwDecode: "ハードウェア デコード、低 CPU",
      cropRotate: "ドラッグ切り抜きと回転",
      playlists: "プレイリスト",
      slideshow: "画像スライドショー",
      library: "壁紙ライブラリ",
      catalog: "内蔵の壁紙カタログ",
      perDisplay: "ディスプレイごとの壁紙（GUI）",
      schedules: "昼夜スケジュール",
      maintained: "活発にメンテナンス中",
      foss: "無料・オープンソース",
    },
    cells: {
      partial: "一部",
      manual: "手動",
      compositorOff: "合成無効時のみ",
      cropOnly: "切り抜きのみ",
      workshop: "Workshop",
    },
  },

  whatsNew: {
    kicker: (version: string) => `新着 · v${version}`,
    title: "壁紙に描かれる、4 つのデスクトップ ウィジェット。",
    lead: (version: string) =>
      `v${version} でリリースされた内容です。追加のウィンドウはなく、クリックの邪魔もせず、X11 でも layer-shell でも同じように動きます。4 つとも既定でオフ。音楽を再生してすべてオンにした状態での実測値は、CPU 1 コアの 0.8% でした。ここに載せた項目はすべて GitHub の CHANGELOG にも記載しています。`,
    changelog: "変更履歴の全文",
    patch: (n: string) => `パッチ ${n}`,
    items: {
      lyrics: {
        title: "同期する歌詞",
        body: "MPRIS で再生中の曲に合わせて、いま歌われている行を表示。まずローカルの .lrc、次に LRCLIB。4 つのプリセットと同期オフセット付き。",
      },
      clock: {
        title: "時計、6 テーマ",
        body: "Digital、Minimal、Segment、Stacked、Wordy、そして描画されたアナログ文字盤を載せた半透明パネルの Card。12/24 時間表示、日付表示は任意。",
      },
      visualizer: {
        title: "オーディオ ビジュアライザー",
        body: "Bars、Mirror、Wave、Dots、Ring から選択。カラー ピッカー、2 色ブレンド、レインボーに対応。音声を聴く前に必ず確認します。",
      },
      disc: {
        title: "レコード上のアルバム アート",
        body: "再生中の曲のジャケットが回転するディスクに。再生を止めた瞬間、回転も止まります。",
      },
    },
  },

  howItWorks: {
    kicker: "使い方",
    title: "3 クリック、あとは忘れていい。",
    lead: "Fresco を開き、追加をクリック、設定をクリック、閉じる。あとはデーモンが壁紙を再生し続けます。再起動しても同じです。",
    step: (n: string) => `ステップ ${n}`,
    steps: {
      pick: {
        title: "メディアを選ぶ",
        description:
          "アプリ メニューから Fresco を開き、動画・GIF・画像・フォルダー・プレイリストを選びます。",
      },
      set: {
        title: "設定をクリック",
        description:
          "壁紙として設定します。すぐにデスクトップで再生が始まります。",
      },
      close: {
        title: "アプリを閉じる",
        description:
          "ウィンドウを閉じます。軽量なデーモンが壁紙を再生し続け、再起動後も復元します。",
      },
    },
  },

  videos: {
    kicker: "動いているところ",
    title: "どれも 1 分未満。ナレーションは不要です。",
    lead: "実機のデスクトップで撮った Fresco の短い画面録画です。再生を押すまで YouTube からは何も読み込まれません。",
    more: "YouTube でもっと見る",
    inDevelopment: "開発中",
    play: (title: string) => `再生: ${title}`,
    items: {
      "YWzD3-xkCEc": {
        tag: "リンクから追加",
        blurb:
          "Pinterest のリンクをコピーして Fresco に貼り付け、そのまま壁紙に設定。ダウンロードもファイル管理も不要です。",
      },
      C1MqrhGkovQ: {
        tag: "歌詞ウィジェット",
        blurb:
          "Wayland と X11 のライブ壁紙に描画される、同期歌詞と時計。オーディオ ビジュアライザーとアルバム アート ディスクとともに v1.1.36 でリリースされました。",
      },
    },
  },

  supported: {
    kicker: "動作環境",
    title: "Fresco が動く場所。",
    lead: "Deepin 25 の DDE を含むあらゆる X11 デスクトップと、Wayland の layer-shell コンポジタ（COSMIC、Hyprland、Sway、KDE Plasma 6）で、主要な Debian 系・Ubuntu 系ディストリビューションに対応します。GNOME Wayland では静止フレームにフォールバックします。",
    deployed: (distros: number, formats: number) =>
      `動作実績: ライブ対応コンポジタ 6 · 静止フォールバック 1 · ディストリ ${distros} · 形式 ${formats}`,
    sessionsTitle: "セッションとコンポジタ",
    distrosTitle: (n: number) => `検証済みディストリビューション · ${n}`,
    formatsTitle: (n: number) => `対応形式 · ${n}`,
    live: "ライブ壁紙",
    fallback: "静止フォールバック",
    sessions: {
      x11: {
        label: "X11（すべてのデスクトップ）",
        detail: "GNOME、KDE、XFCE、MATE、Cinnamon、Budgie",
      },
      deepin: {
        label: "Deepin 25（DDE、X11）",
        detail:
          "DDE に自動で適応し、アイコンは表示されたまま。Deepin 25 Community build1 でコミュニティ検証済み。",
      },
      wayland: {
        label: "Wayland layer-shell",
        detail: "COSMIC、Hyprland、Sway、KDE Plasma 6、wlroots",
      },
      gnome: {
        label: "GNOME on Wayland",
        detail: "静止フレームへのフォールバック（Mutter にライブ サーフェスがないため）",
      },
    },
    fieldReport: "実地レポート · deepin 25",
    verifiedEnv: "検証環境",
    testimonialRole: "Deepin コミュニティのテスター",
    envLabels: {
      session: "セッション",
      os: "os",
      gpu: "gpu",
    },
    footnote:
      "deepin 25 の既定は x11 で、fresco が検証されているのもそのセッションです。deepin 独自の wayland コンポジタ treeland はまだ開発中のため、fresco は deepin の wayland 環境については現時点で何も主張していません。",
  },

  download: {
    kicker: "ダウンロード",
    title: "Debian、Ubuntu、Pop!_OS、Mint に導入。",
    badge: "x11 · wayland",
    lead: "公式のワンライナー インストーラー、または .deb リリースのどちらでも。クリップボードにコピーすればすぐ実行できます。ウィンドウを閉じても Fresco は再生を続けます。",
    cardTitle: "ワンライナー インストール",
    cardBody:
      "ターミナルで実行してください。常に最新の .deb をダウンロードしてインストールします:",
    terminalTitle: "fresco install",
    aptComment: ".deb をすでにダウンロード済みですか？",
    releases: "すべてのリリースを見る",
    gpuNote:
      "CPU 使用率を最小にするには、GPU のハードウェア デコード ドライバー（Intel media VA ドライバー、Mesa VA ドライバー、または NVDEC 用の NVIDIA プロプライエタリ ドライバー）をインストールしてください。",
    copy: "コピー",
    copied: "コピーしました",
  },

  faq: {
    kicker: "よくある質問",
    title: "疑問に、お答えします。",
    lead: "Linux で最初のライブ壁紙を設定する前に知っておきたいことをまとめました。",
    items: [
      {
        q: "Linux 版の Wallpaper Engine はありますか？",
        a: "あります。Fresco は Wallpaper Engine と同じことができる、Linux 向けの無料オープンソースのライブ壁紙アプリです。動画・GIF・画像を選んで、動くデスクトップ背景として設定できます。GUI 中心の設計で、Steam も Proton も必要ありません。",
      },
      {
        q: "Ubuntu や Pop!_OS で動画を壁紙にするには？",
        a: "Fresco の .deb をインストールし、アプリ メニューから起動して「追加」をクリック、動画を選び、必要なら切り抜きや回転をしてから「壁紙に設定」をクリックします。アプリを閉じても、その動画はデスクトップ背景として再生され続けます。",
      },
      {
        q: "動画の壁紙は CPU やバッテリーを消耗しませんか？",
        a: "しません。Fresco は mpv 経由で GPU（VA-API と NVDEC）で動画をデコードするため、CPU 使用率はほぼゼロ、メモリは 120〜150 MB 程度です。バッテリー駆動中は自動で一時停止でき、全画面ウィンドウのあるモニターでも自動的に停止します。",
      },
      {
        q: "Fresco は Wayland や COSMIC デスクトップで動きますか？",
        a: "動きます。Fresco は同梱・監視付きの mpvpaper バックエンドを通じて、Wayland の layer-shell コンポジタで動く壁紙を再生します。COSMIC（Pop!_OS 24.04）、Hyprland、Sway、KDE Plasma 6、その他の wlroots 系コンポジタが対象です。v1.1.1 以降は mpvpaper を 2 種類同梱して実行時に判定するため、libmpv1 と libmpv2 のどちらのディストリビューションでも動作します。X11 ではどのデスクトップでも動きます。",
      },
      {
        q: "Fresco は GNOME で動きますか？",
        a: "GNOME の X11 セッションなら、ライブ壁紙がすべて動きます。GNOME の Wayland では Mutter がライブ壁紙のサーフェスを提供しないため、動いているふりをするのではなく、選んだ壁紙の静止フレームを表示するフォールバックになります。",
      },
      {
        q: "動画の壁紙から音は出せますか？",
        a: "出せます。壁紙ごとにミュート状態と音量を記憶するので、特定の動画だけミュートを解除しておけば、設定するたびにその状態が復元されます。既定ではミュートで再生されます。",
      },
      {
        q: "壁紙を切り抜いたり回転したりできますか？",
        a: "できます。エディターにはドラッグで範囲を決める切り抜き枠と 90 度回転があるので、見せたい部分だけを選んだり、横向きに撮ったスマホ動画を正しい向きにしたりできます。どちらも GPU 上で適用され、壁紙ごとに記憶されます。",
      },
      {
        q: "再起動しても壁紙は残りますか？",
        a: "残ります。Fresco は自動起動エントリを追加してログイン時にライブ壁紙を復元し、エントリが失われていれば自動で修復します。設定でオフにすることもできます。",
      },
      {
        q: "対応しているメディア形式は？",
        a: "ループ再生する動画（mp4、webm、mkv、avi、mov）、アニメーション GIF、静止画（jpg、png、webp）、画像フォルダーをクロスフェード・フェード・スライド・Ken Burns のトランジション付きスライドショーとして再生、そして複数動画のプレイリストです。",
      },
      {
        q: "マルチモニターに対応していますか？",
        a: "対応しています。ディスプレイごとに別の壁紙を設定でき、あるモニターでウィンドウが全画面になると、そのモニターの壁紙だけを一時停止します。モニターのホットプラグは X11 ではその場で反映され、Wayland では新しく接続したディスプレイは次回の適用時に反映されます（自動ホットプラグは v1.0 エンジンで対応予定）。",
      },
      {
        q: "Fresco は Wallpaper Engine とどう違いますか？",
        a: "Wallpaper Engine は有料の Windows 優先製品で、Linux では Steam Play と Proton 経由でしか動きません。Fresco は無料でオープンソース（GPL-3.0）、そして Linux ネイティブです。Steam も Proton も互換レイヤーも不要。Steam Workshop の代わりに、厳選されたライセンス済み壁紙の内蔵カタログがあり、X11 と Wayland layer-shell コンポジタに直接対応します。",
      },
      {
        q: "Fresco は Hidamari や Komorebi、mpvpaper とどう違いますか？",
        a: "Fresco は GUI 中心でハードウェア アクセラレーション対応、動画・GIF・画像・スライドショー・プレイリストを 1 つのアプリで、X11 でも Wayland でも扱えます。Komorebi と違って活発にメンテナンスされており、mpvpaper と違ってコマンドラインは不要です。",
      },
      {
        q: "Linux 用のライブ壁紙はどこで手に入りますか？",
        a: "Fresco の中にあります。内蔵カタログ（メニュー →「壁紙を閲覧」）には、厳選され適切にライセンスされた動画壁紙が並び、2 クリックで設定できます。各項目にはライセンスと作者が表示されます。動画や画像の直接 URL を貼り付けたり、自分のファイルを追加したりもできます。",
      },
      {
        q: "昼と夜で壁紙を自動的に切り替えられますか？",
        a: "できます。メニューから「詳細設定」→「昼夜の壁紙」を開き、2 つの壁紙と切り替え時刻を選ぶと、デーモンが再起動なしで自動的に入れ替えます。任意の時間帯や、日の出・日の入り連動（座標は手動指定）は config.toml から設定できます。",
      },
      {
        q: "モニターごとに別の壁紙を設定するには？",
        a: "ライブラリで壁紙を右クリックし、「特定のディスプレイに設定」を選びます。接続中のモニターが解像度付きで一覧表示されます。「すべてのディスプレイで既定を表示」を選ぶと、モニターごとの上書きが解除されます。",
      },
      {
        q: "Linux のデスクトップに歌詞を表示できますか？",
        a: "できます。Fresco は MPRIS 経由でシステム上の再生（ブラウザー、音楽アプリ、動画プレイヤー）を追いかけ、時間同期した歌詞を壁紙に描画します。4 つのプリセット、9 分割の配置グリッド、同期オフセットのスライダー、次の行の表示（任意）、曲名とアーティストの表示（任意）があります。歌詞はまずローカルの .lrc ファイル、次にコミュニティ運営の無料データベース LRCLIB から取得します。プレイヤーとしては Firefox が最も安定しています。Spotify のネイティブ Linux クライアントは再生位置を正しく報告しませんが、ブラウザー版の Spotify なら問題ありません。",
      },
      {
        q: "Linux に Conky のようなデスクトップ ウィジェットはありますか？",
        a: "あります。しかも Fresco が追加する 4 つは、パネルも拡張機能もデスクトップ側の対応も必要としません。同期する歌詞、6 テーマの時計、5 スタイルのオーディオ ビジュアライザー、そして回転するレコード上のジャケットです。ウィンドウではなく壁紙そのものに描かれるので、ウィンドウの上に重なることも、クリックを奪うこともなく、COSMIC・Hyprland・Sway のようにウィジェット層を持たないデスクトップでも動きます。4 つとも既定でオフです。Conky と違い、システム モニター系のウィジェットはまだないため、CPU・RAM・ネットワークの表示はできません。ウィジェットが動かない唯一の環境は GNOME on Wayland で、描画先となるライブ壁紙のサーフェスが存在しないためです。",
      },
      {
        q: "デスクトップ背景に音楽ビジュアライザーを表示できますか？",
        a: "できます。Fresco のオーディオ ビジュアライザーはシステムで再生中の音に反応し、5 つのスタイル（Bars、Mirror、Wave、Dots、Ring）から選べます。カラー ピッカー、2 色ブレンド、レインボーにも対応します。既定ではオフで、音声出力を聴く必要があるため、初回に有効化するときは必ず同意を求めます。音楽を再生して 4 つのウィジェットをすべてオンにした状態での実測値は CPU 1 コアの 0.8% で、そのほとんどは音声キャプチャです。内容が変わらない限り再描画しないためです。",
      },
      {
        q: "Fresco は無料ですか？",
        a: "無料です。Fresco は GPL-3.0 ライセンスの完全に無料なオープンソース ソフトウェアです。有料プランはありません。",
      },
    ],
  },

  footer: {
    github: "GitHub",
    license: "ライセンス",
    tagline: "rust + gtk4 + mpv",
    sound: "サウンドを切り替え",
  },

  featureList: [
    "厳選されたライセンス済み壁紙の内蔵カタログ",
    "動画・GIF・画像・スライドショー・プレイリストの壁紙",
    "壁紙に描画されるデスクトップ ウィジェット: 同期歌詞、時計、オーディオ ビジュアライザー、アルバム アート",
    "直接 URL からの壁紙追加",
    "昼夜の壁紙スケジュール（設定ファイルで時間帯や日の出・日の入りにも対応）",
    "GUI からのディスプレイごとの壁紙設定",
    "サウンド サーバーの起動が遅れた場合の音声自動復旧",
    "スクリプト可能な JSON 制御ソケット",
    "ハードウェア アクセラレーション再生（VA-API、NVDEC）",
    "X11 と Wayland layer-shell コンポジタで動作",
    "ドラッグ切り抜きと 90 度回転のエディター",
    "壁紙ごとの音声と音量",
    "スライドショーのトランジション（クロスフェード、フェード、スライド、Ken Burns）",
    "検索付きの壁紙ライブラリ",
    "モニターごとに異なる壁紙",
    "バッテリー時の一時停止と全画面時の自動停止",
    "ログイン時の自動復元",
    "テーマとアクセント カラー",
  ],

  softwareDescription:
    "Fresco は Linux 向けの無料オープンソースのライブ壁紙アプリです。動画・GIF・画像・スライドショー・プレイリストを、ハードウェア アクセラレーション再生で動くデスクトップ背景として設定でき、さらに 4 つのデスクトップ ウィジェット（時間同期の歌詞、時計、オーディオ ビジュアライザー、回転するレコード上のアルバム アート）を壁紙そのものに描画できます。Pop!_OS、Ubuntu、Linux Mint、Debian、elementary OS 向けの無料の Wallpaper Engine 代替で、X11 と Wayland layer-shell コンポジタ（COSMIC、Hyprland、Sway、KDE Plasma 6）に対応します。",
};
