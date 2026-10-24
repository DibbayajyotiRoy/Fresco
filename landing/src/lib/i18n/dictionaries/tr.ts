import type { Dictionary } from "./en";

/**
 * Türkçe. Ürün adları ve platform terimleri İngilizce kalır (Fresco, X11,
 * Wayland, layer-shell, mpv, GPL-3.0 ...), Türkçe Linux belgelerindeki gibi.
 * "Siz" hitabı kullanılır. Arayüz terimleri uygulamanın kendi Türkçe
 * kataloğuyla (i18n/tr.json) uyumludur: kitaplık, monitör, ekran, widget,
 * slayt gösterisi, çalma listesi, sabit kare. Aramalarda yazıldığı gibi,
 * gövde metninde "duvar kağıdı" yazılır; tırnak içindeki arayüz etiketleri
 * ise uygulamadaki yazımıyla ("duvar kâğıdı") birebir alınmıştır.
 */
export const tr: Dictionary = {
  meta: {
    title:
      "Fresco - Linux için Canlı Duvar Kağıdı | Ücretsiz Wallpaper Engine Alternatifi",
    description:
      "Linux için ücretsiz, açık kaynaklı canlı duvar kağıdı uygulaması. Yerleşik katalog, video ve GIF’ten hareketli duvar kağıdı, monitör başına duvar kağıdı, gündüz/gece zamanlaması. X11 ve Wayland’de donanım hızlandırmalı.",
    ogTitle: "Fresco - Linux için Canlı Duvar Kağıtları",
    ogDescription:
      "Yerleşik duvar kağıdı kataloğu, monitör başına duvar kağıdı, gündüz/gece zamanlaması ve X11 ile Wayland’de işlemciyi neredeyse hiç yormayan, donanım hızlandırmalı oynatma. Ücretsiz bir Wallpaper Engine alternatifi.",
    twitterDescription:
      "Linux için donanım hızlandırmalı canlı duvar kağıtları, X11 ve Wayland’de. Ücretsiz, açık kaynaklı bir Wallpaper Engine alternatifi.",
    ogImageAlt: "Fresco. Sonunda sorunsuz çalışan bir Linux duvar kağıdı.",
    /** Locale-specific search terms, appended to the shared English set. */
    keywords: [
      "canlı duvar kağıdı",
      "linux canlı duvar kağıdı",
      "hareketli duvar kağıdı",
      "linux hareketli duvar kağıdı",
      "video duvar kağıdı linux",
      "wallpaper engine linux alternatifi",
      "ubuntu canlı duvar kağıdı",
      "kali linux canlı duvar kağıdı",
    ],
  },

  nav: {
    home: "Fresco ana sayfa",
    features: "Özellikler",
    compare: "Karşılaştır",
    whatsNew: "Yenilikler",
    download: "İndir",
    cta: "Fresco’yu al",
    star: "GitHub’da Fresco’ya yıldız verin",
    starWithCount: (n: string) => `GitHub’da Fresco’ya yıldız verin (${n} yıldız)`,
    menu: "Menü",
  },

  language: {
    label: "Dil",
    change: "Dili değiştir",
  },

  theme: {
    toggle: "Temayı değiştir",
    light: "Açık",
    dark: "Koyu",
  },

  hero: {
    titleLead: "Sonunda, sorunsuz çalışan",
    titleGap: " ",
    titleEm: "bir Linux duvar kağıdı.",
    body: "Herhangi bir videoyu, GIF’i ya da görseli Linux masaüstünüze duvar kağıdı yapın. Donanım hızlandırmalı oynatma, X11 ve Wayland’de işlemci kullanımını sıfıra yakın tutar. Uygulamayı kapatın; arka plan hizmeti oynatmaya devam eder.",
    install: "Fresco’yu kur",
    star: "GitHub’da yıldızla",
    sendToComputer: "Bilgisayarıma gönder",
    sendToComputerShareTitle: "Fresco",
    sendToComputerShareText:
      "Fresco: Linux için ücretsiz, açık kaynaklı bir canlı duvar kağıdı uygulaması.",
    linkCopied: "Bağlantı kopyalandı. Linux bilgisayarınızda açın.",
    howToInstall: "Nasıl kurulur",
    newUsers24h: (n: string) =>
      `${n} kişi son 24 saatte Fresco’yu kullanmaya başladı`,
    builtBy: "Geliştiren:",
    activeToday: (n: string) => `${n} kişi son 24 saatte Fresco’yu kullandı`,
    newToday: (n: string) => `${n} yeni`,
    desktopActivities: "Etkinlikler",
    desktopClock: "10:42",
    desktopIconHome: "Ev",
    desktopIconPictures: "Resimler",
    desktopIconDocument: "Notlar",
    desktopIconTrash: "Çöp",
  },

  stats: {
    ariaLabel: "Proje istatistikleri",
    downloads: "toplam indirme",
    downloadsUnknown: "GitHub’da indirme",
    stars: "GitHub yıldızı",
    version: "Son sürüm",
    license: "ücretsiz ve açık kaynak",
    users: "kullanıcı",
    countries: "ülke",
    cohortNote:
      "Kullanıcı ve ülke sayıları, anonim telemetriye izin veren kurulumlardan hesaplanır.",
    active24h: "son 24 saatte etkin",
  },

  testimonials: {
    kicker: "Kullanıcı notları",
    title: (users: string, countries: string) =>
      `${countries} ülkede ${users} Linux masaüstünde çalışıyor.`,
    lead: "Fresco kullanan kişilerden, uygulamanın içinden gönderilmiş, düzenlenmemiş notlar. Anonim geri bildirimler yalnızca ülkeyle anılır.",
    fromCountry: (country: string) => `Fresco kullanıcısı · ${country}`,
    anonymous: "Fresco kullanıcısı",
    namedLabel: "Kullanıcı notu",
    pause: "Yorumları duraklat",
    play: "Yorumları oynat",
  },

  glance: {
    ariaLabel: "Bir bakışta Fresco",
    caption: "Bir bakışta Fresco",
    labelWhat: "Nedir",
    labelPlatforms: "Platformlar",
    labelWidgets: "Masaüstü widget’ları",
    labelLicense: "Lisans",
    labelInstall: "Kurulum",
    what: "Fresco, Linux için ücretsiz ve açık kaynaklı bir canlı duvar kağıdı uygulamasıdır: video, GIF, görsel, slayt gösterisi ve çalma listesi duvar kağıtlarını, GPU donanım kod çözmesiyle hareketli masaüstü arka planı yapar. Ücretsiz bir Wallpaper Engine alternatifi ve Wayland’de mpvpaper için bir arayüzdür.",
    platforms:
      "Her X11 masaüstü (Ubuntu, Pop!_OS, Linux Mint, Debian, Kali Linux) ve Wayland layer-shell compositor’ları: COSMIC, Hyprland, Sway, KDE Plasma 6. GNOME Wayland’de, bir Fresco GNOME eklentisi çıkana kadar yalnızca sabit kare gösterilir.",
    widgets:
      "Bir pencereye değil, doğrudan duvar kağıdının kendisine çizilen beş widget: zamanla eşleşen şarkı sözleri, sekiz temalı bir saat, bir dünya saati, bir ses görselleştirici ve dönen bir plakta albüm kapağı. Dünya saatinin isteğe bağlı “Pencerelerin üzerinde dursun” modunu açmadıkça pencerelerinizin üzerinde hiçbir şey durmaz ve hiçbir tıklamayı engellemez. Hepsi varsayılan olarak kapalıdır; canlı duvar kağıdı yüzeyi olmayan GNOME Wayland’de kullanılamaz.",
    licenseLead: "GPL-3.0, sonsuza dek ücretsiz.",
    licenseLink: "GitHub’da kaynak kodu",
    licenseTail: "Rust, GTK4 ve mpv ile geliştirildi.",
  },

  features: {
    kicker: "Özellikler",
    title: "Her medya. Her monitör. İşlemci derdi yok.",
    lead: "Fresco; video, GIF, görsel, slayt gösterisi ve çalma listesi duvar kağıtlarını X11 ve Wayland’de ayarlar. Kod çözme GPU’da yapıldığı için canlı bir duvar kağıdı, sabit bir duvar kağıdına yakın bir maliyetle çalışır.",
    widgetsTitle: "Beş masaüstü widget’ı, doğrudan duvar kağıdına çizilir.",
    worldClock: {
      title: "Dünya saati",
      body: "Aynı anda birkaç şehir; arkasında kart olmayan, hafif gölgeli, sade 12 saatlik metin olarak. İstediğiniz saat dilimlerini seçin ve her birini adlandırın. Dakikada bir yeniden çizilir; isteğe bağlı bir mod onu pencerelerinizin üzerinde tutar ve tıklamalar içinden geçer.",
    },
    manifest: (n: number) => `Manifest: ${n} özellik`,
    titleSuffix: ".",
    thCapability: "Özellik",
    thWhatYouGet: "Ne elde edersiniz",
    thStatus: "Durum",
    footnote:
      "GNOME Wayland: yalnızca sabit kare. Mutter canlı bir yüzey sunmaz; GNOME 50, Ubuntu 25.10+ ve Fedora 43+ ise geri dönülecek bir Xorg oturumu sunmaz. Widget’lar da bu yüzeye ihtiyaç duyar, bu yüzden orada kullanılamazlar; canlı video ise planlanan bir Fresco GNOME eklentisini bekliyor. Yukarıdaki diğer her şey canlıdır.",
    tally: (shipping: number, total: number, soon: number) =>
      `${total} özellikten ${shipping} tanesi yayında · ${soon} önizlemede · 0 kullanımdan kaldırılmış`,
    rows: {
      hwDecode: {
        tag: "Donanım kod çözme",
        title: "Donanım hızlandırmalı oynatma",
        description:
          "Kod çözme, mpv üzerinden GPU’da çalışır (VA-API veya NVDEC). 4K bir video duvar kağıdı, sabit bir görsel kadar işlemci kullanır.",
        status: "İşlemci neredeyse boşta",
      },
      sessions: {
        tag: "Oturumlar",
        title: "X11 ve Wayland",
        description:
          "Her X11 masaüstünde bir masaüstü penceresi arka ucu, ayrıca COSMIC, Hyprland, Sway ve KDE Plasma 6 için bir layer-shell arka ucu. GNOME Wayland, planlanan GNOME eklentisine kadar sabit kare gösterir.",
        status: "X11 · layer-shell",
      },
      catalog: {
        tag: "Katalog",
        title: "Yerleşik duvar kağıdı kataloğu",
        description:
          "Derlenmiş, lisanslı duvar kağıtlarına uygulama içinden göz atın (menü, ardından “Duvar kâğıtlarına göz at”) ve iki tıkla birini ayarlayın. Doğrudan bir bağlantı da yapıştırabilirsiniz.",
        status: "Uygulama içinde",
      },
      linkedFolders: {
        tag: "Klasörler",
        title: "Klasör bağla",
        description:
          "Zaten sahip olduğunuz bir duvar kağıdı klasörünü (görseller, GIF’ler ve videolar) içe aktarmadan kullanın. Dosya ekleyip kaldırdıkça eşitlenir, alt klasörler isteğe bağlıdır ve favorileriniz ile düzenlemeleriniz dosya başına saklanır.",
        status: "İçe aktarma yok",
      },
      bigLibrary: {
        tag: "Kitaplık",
        title: "Binlerce duvar kağıdı",
        description:
          "Kitaplık yalnızca ekrandakini çizer. 4.000 duvar kağıtlık bir test kitaplığı yaklaşık 2 saniyede açıldı ve yaklaşık 325 MB kullandı; önceden bu değerler 9 ile 13 saniye ve 1,6 GB idi.",
        status: "İhtiyaç anında",
      },
      video: {
        tag: "Video · GIF",
        title: "Video ve GIF duvar kağıtları",
        description:
          "Herhangi bir mp4, webm, mkv veya hareketli GIF’i masaüstünüzde döngüye alın.",
        status: "mp4 webm mkv gif",
      },
      slideshow: {
        tag: "Slayt gösterisi",
        title: "Geçişli slayt gösterileri",
        description:
          "Bir görsel klasörünü siyaha solarak geçiş, Ken Burns, yakınlaştırma veya bulanıklıkla döndürün.",
        status: "4 geçiş",
      },
      playlist: {
        tag: "Çalma listesi",
        title: "Video çalma listeleri",
        description:
          "Birkaç klibi sıraya alın, Fresco hepsini sırayla oynatsın.",
        status: "Otomatik döngü",
      },
      lyrics: {
        tag: "Şarkı sözü · saat",
        title: "Şarkı sözü ve saat widget’ları",
        description:
          "MPRIS üzerinden çalan her şey için zamanla eşleşen şarkı sözleri (önce yerel .lrc, sonra LRCLIB), sekiz temadan birinde bir saat ve aynı anda birkaç şehri gösteren bir dünya saati. Duvar kağıdına çizilir; dünya saatinin “Pencerelerin üzerinde dursun” modunu açmadıkça hiçbir şey pencerelerinizin üzerinde durmaz. Varsayılan olarak kapalı.",
        status: "Varsayılan olarak kapalı",
      },
      visualiser: {
        tag: "Görselleştirici",
        title: "Ses görselleştirici ve albüm kapağı",
        description:
          "Renk seçici, karışım veya gökkuşağı seçenekleriyle beş görselleştirici stili (Çubuklar, Ayna, Dalga, Noktalar, Halka) ve çalan parçanın kapağını dönen bir plakta gösterme. Görselleştirici, sesinizi dinlemeden önce sorar.",
        status: "Bir çekirdeğin %0,8’i",
      },
      editor: {
        tag: "Düzenleyici",
        title: "Kırpma ve döndürme",
        description:
          "Bölgeyi seçmek için bir çerçeve sürükleyin, yan duran klipleri düzeltmek için 90 derece döndürün. İkisi de GPU’da sıfır kopyayla çalışır.",
        status: "Sıfır kopya",
      },
      audio: {
        tag: "Ses",
        title: "Duvar kağıdı başına ses",
        description:
          "Bir videonun sesini açın ve ses düzeyini ayarlayın. Fresco bu seçimi o duvar kağıdı için hatırlar.",
        status: "Duvar kağıdı başına",
      },
      displays: {
        tag: "Ekranlar",
        title: "Ekran başına duvar kağıdı",
        description:
          "Herhangi bir duvar kağıdına sağ tıklayıp belirli bir ekranda ayarlayın. Her monitör kendi duvar kağıdını oynatabilir.",
        status: "Monitör başına",
      },
      schedule: {
        tag: "Zamanlama",
        title: "Gündüz ve gece zamanlaması",
        description:
          "İki duvar kağıdı, iki geçiş saati; arka plan hizmeti bunları otomatik değiştirir. Zaman dilimleri ve gün doğumu/batımına göre geçiş yapılandırma dosyasıyla.",
        status: "Otomatik",
      },
      power: {
        tag: "Güç",
        title: "Güç durumuna duyarlı",
        description:
          "Pilde duraklatır ve bir monitörde tam ekran pencere açıldığında o monitör için otomatik duraklatır.",
        status: "Otomatik duraklatma",
      },
      newTab: {
        tag: "Tarayıcı yeni sekmesi",
        title: "Duvar kağıdınız her yeni sekmede",
        description:
          "Eşlik eden bir tarayıcı eklentisi (Chrome, Brave, Edge, Firefox), yalnızca 127.0.0.1 ile konuşan yerel bir köprü üzerinden masaüstü duvar kağıdınızı ya da tarayıcıya özel bir seçimi yeni sekme sayfasında yansıtır. Bugün depoda mevcut; mağaza listelemeleri bekliyor.",
        status: "Yakında",
      },
      themes: {
        tag: "Temalar",
        title: "Temalar ve vurgu renkleri",
        description:
          "Açık, koyu ya da sistemi izle; altı vurgu paleti.",
        status: "6 palet",
      },
    },
  },

  compare: {
    kicker: "Karşılaştır",
    title: "Fresco ve Linux duvar kağıdı alternatifleri.",
    lead: "Fresco, bu tabloda arayüzü, donanım kod çözmeyi, X11 ve Wayland desteğini ve yerleşik kataloğu bir arada, ücretsiz sunan, aktif olarak bakımı yapılan tek Linux canlı duvar kağıdı uygulamasıdır. Hidamari, Komorebi, mpvpaper ve Wallpaper Engine ile tam karşılaştırma aşağıda.",
    meter: (tools: number, caps: number) =>
      `Karşılaştırma · ${tools} araç · ${caps} özellik`,
    thFeature: "Özellik",
    yes: "Evet",
    no: "Hayır",
    note: "Wallpaper Engine ücretli ve önce Windows için geliştirilmiş bir üründür. Komorebi artık bakım görmüyor.",
    detailLabel: "Ayrıntılı karşılaştırma:",
    vs: (tool: string) => `Fresco ve ${tool}`,
    rows: {
      gui: "Arayüzlü uygulama, terminal gerekmez",
      x11: "X11’de çalışır",
      wayland: "Wayland’de çalışır (layer-shell)",
      hwDecode: "Donanım kod çözme, düşük işlemci",
      cropRotate: "Sürükleyerek kırpma ve döndürme",
      playlists: "Çalma listeleri",
      slideshow: "Görsel slayt gösterisi",
      library: "Duvar kağıdı kitaplığı",
      catalog: "Yerleşik duvar kağıdı kataloğu",
      perDisplay: "Ekran başına duvar kağıdı (arayüzden)",
      schedules: "Gündüz ve gece zamanlaması",
      maintained: "Aktif olarak bakımı yapılıyor",
      foss: "Ücretsiz ve açık kaynak",
    },
    cells: {
      partial: "Kısmi",
      manual: "Elle",
      compositorOff: "Compositor kapalı",
      cropOnly: "Yalnızca kırpma",
      workshop: "Atölye",
      unclear: "Belirsiz",
      na: "yok",
    },
  },

  whatsNew: {
    kicker: (version: string) => `Yenilikler · v${version}`,
    title: "Dört masaüstü widget’ı, doğrudan duvar kağıdına çizilir.",
    lead: (version: string) =>
      `v${version} ile gelenler. Ek pencere yok, içinden tıklanacak bir şey yok, X11 ve layer-shell’de aynı. Dördü de varsayılan olarak kapalı; müzik çalarken ve hepsi açıkken ölçülen maliyet bir işlemci çekirdeğinin %0,8’iydi. Buradaki her madde GitHub’daki CHANGELOG’da da yer alır.`,
    changelog: "Tüm değişiklik günlüğü",
    patch: (n: string) => `Yama ${n}`,
    items: {
      lyrics: {
        title: "Senkronize şarkı sözleri",
        body: "MPRIS üzerinden çalan her şeyle zamanında eşleşen geçerli satır. Önce yerel .lrc dosyaları, sonra LRCLIB. Dört ön ayar ve bir senkron ayarı.",
      },
      clock: {
        title: "Saat, sekiz tema",
        body: "Dijital, Minimal, Segment, Yığılmış, Yazıyla, Kart, NOS ve telefon tarzı bir Kilit ekranı yüzü. 12 veya 24 saat, isteğe bağlı tarih.",
      },
      visualizer: {
        title: "Ses görselleştirici",
        body: "Çubuklar, Ayna, Dalga, Noktalar veya Halka; renk seçici, iki renkli karışım ya da gökkuşağı ile. Sesinizi dinlemeden önce sorar.",
      },
      disc: {
        title: "Plakta albüm kapağı",
        body: "Çalan parçanın kapağı dönen bir diskte. Oynatma duraklatıldığı anda dönmeyi bırakır.",
      },
    },
  },

  howItWorks: {
    kicker: "Nasıl çalışır",
    title: "Üç tık, sonra unutun gitsin.",
    lead: "Fresco’yu açın, Ekle’ye tıklayın, Ayarla’ya tıklayın, kapatın. Arka plan hizmeti duvar kağıdını, yeniden başlatmadan sonra bile çalışır halde tutar.",
    step: (n: string) => `Adım ${n}`,
    steps: {
      pick: {
        title: "Medyanızı seçin",
        description:
          "Fresco’yu uygulama menüsünden açın ve bir video, GIF, görsel, klasör ya da çalma listesi seçin.",
      },
      set: {
        title: "Ayarla’ya tıklayın",
        description:
          "Duvar kağıdınız olarak ayarlayın. Masaüstünüzde hemen oynamaya başlar.",
      },
      close: {
        title: "Uygulamayı kapatın",
        description:
          "Pencereyi kapatın. Hafif bir arka plan hizmeti duvar kağıdını, yeniden başlatmadan sonra bile çalışır halde tutar.",
      },
    },
  },

  videos: {
    kicker: "Çalışırken izleyin",
    title: "Her biri bir dakikadan kısa. Anlatıma gerek yok.",
    lead: "Fresco’nun gerçek bir masaüstündeki kısa ekran kayıtları. Oynat’a basana kadar YouTube’dan hiçbir şey yüklenmez.",
    more: "YouTube’da daha fazlası",
    inDevelopment: "Geliştiriliyor",
    play: (title: string) => `Oynat: ${title}`,
    items: {
      "YWzD3-xkCEc": {
        tag: "Bağlantıdan ekle",
        blurb:
          "Bir Pinterest bağlantısı kopyalayın, Fresco’ya yapıştırın, duvar kağıdı yapın. İndirme adımı yok, dosya karmaşası yok.",
      },
      C1MqrhGkovQ: {
        tag: "Şarkı sözü widget’ları",
        blurb:
          "Wayland ve X11’de canlı bir duvar kağıdına çizilmiş senkronize şarkı sözleri ve bir saat. Bir ses görselleştirici ve albüm kapağı diskiyle birlikte v1.1.36’da yayınlandı.",
      },
    },
  },

  supported: {
    title: "Fresco nerede çalışır.",
    lead: "deepin 25’in DDE’si dahil her X11 masaüstünde ve Wayland layer-shell compositor’larında (COSMIC, Hyprland, Sway ve KDE Plasma 6), popüler Debian, Ubuntu ve Kali Linux dağıtımlarında. GNOME Wayland şimdilik yalnızca sabit kare gösterir.",
    sessionsTitle: "Oturumlar ve compositor’lar",
    distrosTitle: (n: number) => `Test edilen dağıtımlar · ${n}`,
    formatsTitle: (n: number) => `Desteklenen biçimler · ${n}`,
    live: "Canlı duvar kağıdı",
    fallback: "Sabit yedek",
    sessions: {
      x11: {
        label: "X11 (her masaüstü)",
        detail: "GNOME, KDE, XFCE, MATE, Cinnamon, Budgie",
      },
      deepin: {
        label: "Deepin 25 (DDE, X11)",
        detail:
          "Otomatik DDE uyarlaması, simgeler görünür kalır. deepin 25 Community build1’de topluluk tarafından doğrulandı. deepin Uygulama Mağazası’nda da var.",
      },
      wayland: {
        label: "Wayland layer-shell",
        detail: "COSMIC, Hyprland, Sway, KDE Plasma 6, wlroots",
      },
      gnome: {
        label: "Wayland’de GNOME",
        detail: "Yalnızca sabit kare (Mutter’da canlı yüzey yok)",
      },
    },
    fieldReport: "Kullanıcı notu · deepin 25",
    verifiedEnv: "Doğrulanan ortam",
    testimonialRole: "deepin topluluğu test kullanıcısı",
    envLabels: {
      session: "Oturum",
      os: "İşletim sistemi",
      gpu: "GPU",
    },
    footnote:
      "deepin 25 varsayılan olarak X11 kullanır ve Fresco orada bu oturumda doğrulanmıştır. deepin’in kendi Wayland compositor’ı Treeland hâlâ geliştirilme aşamasındadır; bu yüzden Fresco, deepin’in Wayland’deki durumu hakkında henüz bir iddiada bulunmaz.",
  },

  download: {
    kicker: "İndir",
    title: "Debian, Ubuntu, Pop!_OS, Mint ve Kali’ye kurun.",
    badge: "X11 · Wayland",
    lead: "Resmî tek satırlık kurulum betiği veya .deb sürümü. İki yol da panonuza kopyalanır ve anında çalışır. Pencereyi kapattıktan sonra da Fresco oynatmaya devam eder.",
    worksOnTitle: "Şurada çalışır",
    cardTitle: "Tek satırlık kurulum",
    cardBody:
      "Bunu bir terminalde çalıştırın. En son .deb paketini sizin için indirir ve kurar, her zaman en yeni sürümü:",
    terminalTitle: "fresco install",
    aptComment: ".deb dosyasını zaten indirdiniz mi?",
    releases: "Tüm sürümlere göz atın",
    gpuNote:
      "En düşük işlemci kullanımı için GPU’nuzun donanım kod çözme sürücüsünü kurun (Intel media VA sürücüsü, Mesa VA sürücüleri veya NVDEC için NVIDIA’nın kapalı kaynaklı sürücüsü).",
    storeLabel: "deepin Uygulama Mağazası’nda da var",
    storeDesc:
      "deepin 25’in DDE masaüstü için, simgeleriyle birlikte derlendi ve doğrulandı.",
    storeStep1: "Uygulama Mağazası’nı açın",
    storeStep2: "“Fresco” arayın",
    storeStep3: "Kur’a tıklayın. Güncellemeler mağaza üzerinden gelir.",
    copy: "Kopyala",
    copied: "Kopyalandı",
  },

  faq: {
    kicker: "SSS",
    title: "Sorular, yanıtlar.",
    lead: "Linux’ta ilk canlı duvar kağıdınızı ayarlamadan önce bilmeniz gereken her şey.",
    items: [
      {
        q: "Linux için Wallpaper Engine var mı?",
        a: "Evet. Fresco, Wallpaper Engine gibi çalışan, Linux için ücretsiz ve açık kaynaklı bir canlı duvar kağıdı uygulamasıdır: bir video, GIF ya da görsel seçin ve hareketli masaüstü arka planı olarak ayarlayın. Önce arayüz odaklıdır; Steam ya da Proton gerektirmez.",
      },
      {
        q: "Ubuntu veya Pop!_OS’ta videoyu duvar kağıdı olarak nasıl ayarlarım?",
        a: "Fresco .deb paketini kurun, uygulama menüsünden açın, Ekle’ye tıklayıp videonuzu seçin, isterseniz kırpın ya da döndürün, ardından “Duvar kâğıdı olarak ayarla”ya tıklayın. Uygulamayı kapatın; video masaüstü arka planınız olarak oynamaya devam eder.",
      },
      {
        q: "Video duvar kağıdı işlemcimi ya da pilimi tüketir mi?",
        a: "Hayır. Fresco videoyu mpv üzerinden GPU’da çözer (VA-API ve NVDEC); bu yüzden işlemci kullanımı sıfıra yakın kalır ve bellek kullanımı 120 ila 150 MB civarındadır. Pildeyken otomatik olarak duraklayabilir ve tam ekran pencere bulunan her monitörde kendiliğinden duraklar.",
      },
      {
        q: "Fresco Wayland’de ve COSMIC masaüstünde çalışır mı?",
        a: "Evet. Fresco, hareketli duvar kağıtlarını Wayland layer-shell compositor’larında, birlikte gelen ve denetlenen bir mpvpaper arka ucuyla çalıştırır: COSMIC (Pop!_OS 24.04), Hyprland, Sway, KDE Plasma 6 ve diğer wlroots compositor’ları. v1.1.1’den beri iki mpvpaper derlemesi içerir ve çalışma anında yoklama yapar; böylece hem libmpv1 hem libmpv2 dağıtımlarında çalışır. X11’de her masaüstünde çalışır.",
      },
      {
        q: "Fresco GNOME’da çalışır mı?",
        a: "X11 oturumlu GNOME’da evet, tam canlı duvar kağıdı. Ancak GNOME 49 X11 oturumunu devre dışı bıraktı, GNOME 50 ise kaldırdı; bu yüzden Ubuntu 25.10 ve sonrasında (26.04 LTS dahil) ile Fedora 43 ve sonrasında X11 oturumu yok; Ubuntu 22.04 ve 24.04 hâlâ “Ubuntu on Xorg” sunuyor. Wayland’li GNOME’da Mutter canlı bir duvar kağıdı yüzeyi sunmaz; Fresco bu yüzden seçtiğiniz duvar kağıdının sabit bir karesini gösterir ve hareket ediyormuş gibi davranmak yerine bunu söyler. GNOME Wayland’de canlı video, planlanan bir Fresco GNOME eklentisini gerektirir.",
      },
      {
        q: "Video duvar kağıdı ses çalabilir mi?",
        a: "Evet. Her duvar kağıdı kendi sessiz durumunu ve ses düzeyini hatırlar; böylece belirli bir videonun sesini açabilirsiniz ve bu seçim her ayarlandığında korunur. Duvar kağıtları varsayılan olarak sessiz başlar.",
      },
      {
        q: "Duvar kağıdını kırpabilir veya döndürebilir miyim?",
        a: "Evet. Düzenleyicide sürüklenebilir bir kırpma çerçevesi ve 90 derecelik döndürme bulunur; böylece tam istediğiniz bölgeyi seçebilir ya da yan çekilmiş bir telefon videosunu dikleştirebilirsiniz. İkisi de GPU’da uygulanır ve duvar kağıdı başına hatırlanır.",
      },
      {
        q: "Yeniden başlatınca duvar kağıdı kalır mı?",
        a: "Evet. Fresco, girişte canlı duvar kağıdınızı otomatik geri yükleyen bir otomatik başlatma girdisi ekler ve girdi eksikse kendini onarır. Bunu ayarlardan kapatabilirsiniz.",
      },
      {
        q: "Hangi medya biçimleri destekleniyor?",
        a: "Döngülü video (mp4, webm, mkv, avi, mov), hareketli GIF’ler, sabit görseller (jpg, png, webp), siyaha solarak geçiş, Ken Burns, yakınlaştırma veya bulanıklık geçişli slayt gösterisi olarak bir görsel klasörü ve çok videolu çalma listeleri.",
      },
      {
        q: "Fresco mevcut duvar kağıdı klasörümü içe aktarmadan kullanabilir mi?",
        a: "Evet. Fresco’yu duvar kağıtlarınızı zaten tuttuğunuz bir klasöre yöneltmek için menüden “Klasör bağla…” seçeneğini (ya da Klasör ekle’den sonra “Bu klasörü bağla”yı) seçin. Klasördeki görseller, GIF’ler ve videolar içe aktarılmadan, bulundukları yerde kitaplığınızda kendi bölümlerinde görünür ve o klasöre dosya ekleyip kaldırdıkça kitaplık eşitlenir. “Alt klasörleri dahil et” isteğe bağlıdır ve dört düzeye kadar iner; favorileriniz ve düzenlemeleriniz dosya başına hatırlanır. Klasör kaybolursa, örneğin bir sürücü çıkarılırsa, bölümü “(kullanılamıyor)” olarak işaretli kalır ve kendiliğinden geri gelir. Bölümün menüsündeki “Klasör bağlantısını kaldır” hiçbir dosyayı silmeden bölümü kaldırır. Ekle ise tek tek dosyaları eskisi gibi içe aktarır.",
      },
      {
        q: "Fresco binlerce duvar kağıdını kaldırır mı?",
        a: "Evet. Kitaplık, tüm duvar kağıtları için aynı anda kart oluşturmak yerine yalnızca kaydırdığınız yerin yakınındaki duvar kağıtları için kart oluşturur; bu yüzden büyük bir koleksiyon uygulamayı ağırlaştırmaz. 4.000 duvar kağıtlık bir test kitaplığında (yarısı görsel, yarısı video) kitaplık 9 ila 13 saniye yerine yaklaşık 2 saniyede göründü ve uygulama 1,6 GB yerine kabaca 325 MB bellek kullandı. Bu rakamlar tek bir makinede, yük altındaki bir geliştirme derlemesinden geliyor; söz olarak değil, yol gösterici olarak okuyun.",
      },
      {
        q: "Birden fazla monitörü destekliyor mu?",
        a: "Evet. Her ekranda farklı bir duvar kağıdı ayarlayabilirsiniz ve bir çıkışta pencere tam ekrana geçtiğinde Fresco duvar kağıdını o çıkış için duraklatır. Monitör takıp çıkarma X11’de canlı çalışır; Wayland’de yeni takılan ekran bir sonraki uygulamada devreye girer (otomatik algılama v1.0 motoruyla gelecek).",
      },
      {
        q: "Fresco, Wallpaper Engine’den nasıl farklı?",
        a: "Wallpaper Engine, ücretli ve önce Windows için geliştirilmiş bir üründür; Linux’ta yalnızca Steam Play ve Proton üzerinden çalışır. Fresco ücretsiz, açık kaynaklı (GPL-3.0) ve Linux’a özgüdür: Steam yok, Proton yok, uyumluluk katmanı yok. Steam Atölyesi yerine, derlenmiş ve lisanslı duvar kağıtlarından oluşan yerleşik bir kataloğu vardır ve X11 ile Wayland layer-shell compositor’larını doğrudan destekler.",
      },
      {
        q: "Fresco, Hidamari, Komorebi ve mpvpaper’dan nasıl farklı?",
        a: "Fresco önce arayüz odaklıdır, donanım hızlandırmalıdır ve video, GIF, görsel, slayt gösterisi ile çalma listesi duvar kağıtlarını tek bir uygulamada, hem X11’de hem Wayland’de yönetir. Komorebi’nin aksine aktif olarak bakımı yapılır, mpvpaper’ın aksine komut satırı gerektirmez.",
      },
      {
        q: "Linux için canlı duvar kağıtlarını nerede bulurum?",
        a: "Doğrudan Fresco’nun içinde. Yerleşik katalog (menü, ardından “Duvar kâğıtlarına göz at”), her öğede lisans ve yazar gösterilen, iki tıkla ayarlayabileceğiniz derlenmiş ve düzgün lisanslı video duvar kağıtları sunar. Doğrudan bir video ya da görsel URL’si de yapıştırabilir veya kendi dosyalarınızı ekleyebilirsiniz.",
      },
      {
        q: "Duvar kağıdım gündüz ve gece arasında otomatik değişebilir mi?",
        a: "Evet. Menüyü açın, Gelişmiş’i, ardından “Gündüz ve gece duvar kâğıdı”nı seçin: iki duvar kağıdı ve geçiş saatlerini belirleyin; arka plan hizmeti yeniden başlatmaya gerek kalmadan bunları otomatik değiştirir. Keyfî zaman dilimleri ve gün doğumu ya da batımına göre geçiş (elle girilen koordinatlarla) config.toml üzerinden kullanılabilir.",
      },
      {
        q: "Her monitörde farklı bir duvar kağıdını nasıl ayarlarım?",
        a: "Kitaplıktaki herhangi bir duvar kağıdına sağ tıklayın ve belirli bir ekranda ayarlamayı seçin. Bağlı her monitör çözünürlüğüyle listelenir. “Tüm ekranlarda varsayılanı göster”i seçmek, monitör başına yapılan geçersiz kılmaları temizler.",
      },
      {
        q: "Linux masaüstümde şarkı sözlerini gösterebilir miyim?",
        a: "Evet. Fresco, sisteminizde MPRIS üzerinden çalan her şeyi (tarayıcılar, müzik uygulamaları, video oynatıcılar) takip ederek zamanla eşleşen şarkı sözlerini duvar kağıdınıza çizer. Dört ön ayar, dokuz noktalı bir yerleşim ızgarası, bir senkron kaydırıcısı, isteğe bağlı sonraki satır ve isteğe bağlı parça adı ile sanatçı vardır. Sözler önce yerel bir .lrc dosyasından, sonra topluluk tarafından işletilen ücretsiz bir veritabanı olan LRCLIB’den gelir. En güvenilir oynatıcı Firefox’tur; Spotify’ın yerel Linux istemcisi bozuk bir oynatma konumu bildirir, ancak tarayıcıdaki Spotify sorunsuzdur.",
      },
      {
        q: "Linux’ta Conky gibi masaüstü widget’ları var mı?",
        a: "Evet, Fresco bunlardan beş tane ekler; panel, eklenti ya da masaüstünüzden destek gerektirmezler: senkronize şarkı sözleri, sekiz temalı bir saat, aynı anda birkaç şehri gösteren bir dünya saati, beş stilli bir ses görselleştirici ve çalan parçanın kapağı dönen bir plakta. Bir pencereye değil, doğrudan duvar kağıdının kendisine çizildikleri için pencerelerinizin arkasında durur, hiçbir tıklamayı engellemez ve kendi widget katmanı olmayan masaüstlerinde de, COSMIC, Hyprland ve Sway dahil, çalışır. Tek istisna, dünya saatinin isteğe bağlı “Pencerelerin üzerinde dursun” modudur: onu her uygulamanın üzerine çıkarır ve tıklamalar içinden geçer. Beşi de varsayılan olarak kapalıdır. Conky’nin aksine henüz sistem izleme widget’ı yok; yani işlemci, RAM ya da ağ göstergesi yok. GNOME Wayland, çizilecek canlı bir duvar kağıdı yüzeyi olmadığı için widget’ların çalışamadığı tek yerdir.",
      },
      {
        q: "Masaüstü arka planımda müzik görselleştirici olabilir mi?",
        a: "Evet. Fresco’nun ses görselleştiricisi, sisteminizin çaldığı her şeye tepki verir; beş stilden birinde (Çubuklar, Ayna, Dalga, Noktalar, Halka) ve renk seçici, iki renkli karışım ya da gökkuşağı ile. Varsayılan olarak kapalıdır ve ses çıkışınızı dinlemesi gerektiği için ilk açtığınızda izin ister. Müzik çalarken ve şarkı sözü, saat, görselleştirici ve albüm kapağı widget’ları açıkken ölçülen maliyet bir işlemci çekirdeğinin %0,8’iydi; bunun neredeyse tamamı ses yakalamaydı, çünkü içeriği değişmedikçe hiçbir şey yeniden çizilmez.",
      },
      {
        q: "Fresco deepin Uygulama Mağazası’nda mı?",
        a: "Evet. deepin 25’te Uygulama Mağazası’nı açın, Fresco’yu arayın ve Kur’a tıklayın. deepin Topluluk Uygulama Mağazası’nda yayımlanmıştır; yani güncellemeler mağaza üzerinden gelir. .deb ve tek satırlık kurulum betiği de çalışır.",
      },
      {
        q: "Fresco ücretsiz mi?",
        a: "Evet. Fresco tamamen ücretsizdir ve GPL-3.0 lisansı altında açık kaynaklıdır. Ücretli bir katman yoktur.",
      },
    ],
  },

  report: {
    title: "Sorun bildirin",
    lead: "Bir hata mı buldunuz ya da bir fikriniz mi var? GitHub’da bize yazın.",
    tipBefore: "Masaüstünüzü, Fresco sürümünüzü ve şu komutun çıktısını ekleyin: ",
    tipAfter: ".",
    bug: "Hata bildir",
    feature: "Özellik iste",
  },

  footer: {
    github: "GitHub",
    license: "Lisans",
    reportIssue: "Sorun bildir",
    tagline: "Rust + GTK4 + mpv",
    sound: "Sesi aç/kapat",
  },

  featureList: [
    "Derlenmiş, lisanslı duvar kağıtlarından oluşan yerleşik katalog",
    "Video, GIF, görsel, slayt gösterisi ve çalma listesi duvar kağıtları",
    "Duvar kağıdına çizilen masaüstü widget’ları: senkronize şarkı sözleri, saat, dünya saati, ses görselleştirici, albüm kapağı",
    "Doğrudan bir URL’den duvar kağıdı ekleme",
    "Gündüz ve gece duvar kağıdı zamanlaması (ayrıca yapılandırma ile zaman dilimleri ve gün doğumu/batımı)",
    "Arayüzden ekran başına duvar kağıdı",
    "Ses sunucusu geç başladığında otomatik ses kurtarma",
    "Betiklenebilir JSON denetim soketi",
    "Donanım hızlandırmalı oynatma (VA-API, NVDEC)",
    "X11 ve Wayland layer-shell compositor’larında çalışır",
    "Sürükleyerek kırpma ve 90 derece döndürme düzenleyicisi",
    "Duvar kağıdı başına ses ve ses düzeyi",
    "Slayt gösterisi geçişleri (siyaha solarak geçiş, Ken Burns, yakınlaştırma, bulanıklık)",
    "Aramalı duvar kağıdı kitaplığı",
    "Klasör bağla: mevcut bir duvar kağıdı klasörünü içe aktarmadan kullanın, dosyalar değiştikçe eşitlenir",
    "Binlerce duvar kağıdıyla da hızlı kalan kitaplık",
    "Monitör başına farklı duvar kağıdı",
    "Pilde duraklatma ve tam ekranda otomatik duraklatma",
    "Girişte otomatik geri yükleme",
    "Temalar ve vurgu renkleri",
  ],

  softwareDescription:
    "Fresco, Linux için ücretsiz, açık kaynaklı bir canlı duvar kağıdı uygulamasıdır. Video, GIF, görsel, slayt gösterisi ve çalma listesi duvar kağıtlarını donanım hızlandırmalı oynatmayla hareketli masaüstü arka planı yapar ve duvar kağıdının kendisine beş masaüstü widget’ı çizebilir: zamanla eşleşen şarkı sözleri, bir saat, bir dünya saati, bir ses görselleştirici ve dönen bir plakta albüm kapağı. Pop!_OS, Ubuntu, Linux Mint, Debian, Kali Linux ve elementary OS için, X11’de ve Wayland layer-shell compositor’larında (COSMIC, Hyprland, Sway, KDE Plasma 6) çalışan ücretsiz bir Wallpaper Engine alternatifi.",
};
