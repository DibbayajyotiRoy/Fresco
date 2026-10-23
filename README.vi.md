<div align="center">

<img src="./data/icons/hicolor/256x256/apps/io.github.dibbayajyotiroy.Fresco.png" width="112" alt="Biểu tượng ứng dụng Fresco — ứng dụng hình nền động miễn phí, mã nguồn mở cho Linux" />

# Fresco — Ứng dụng hình nền động và hình nền video miễn phí cho Linux

[English](README.md) | [বাংলা](README.bn.md) | [Deutsch](README.de.md) | [Español](README.es.md) | [Français](README.fr.md) | [हिन्दी](README.hi.md) | [Bahasa Indonesia](README.id.md) | [Português (Brasil)](README.pt-BR.md) | [Română](README.ro.md) | [Русский](README.ru.md) | [Türkçe](README.tr.md) | **Tiếng Việt** | [简体中文](README.zh-CN.md)

**Fresco là ứng dụng hình nền động (live wallpaper) miễn phí, mã nguồn mở dành cho Linux: biến bất kỳ video, GIF hoặc ảnh nào thành hình nền động cho màn hình nền, trên cả X11 lẫn Wayland.** Đây là lựa chọn thay thế Wallpaper Engine chạy native trên Linux, với giao diện đồ họa, phát video tăng tốc bằng GPU và các widget trên màn hình nền.

[![Bản phát hành Fresco mới nhất trên GitHub](https://img.shields.io/github/v/release/DibbayajyotiRoy/fresco?style=flat-square&label=release)](https://github.com/DibbayajyotiRoy/fresco/releases/latest)
[![Giấy phép GPL-3.0-or-later](https://img.shields.io/github/license/DibbayajyotiRoy/fresco?style=flat-square)](LICENSE)
[![Trạng thái workflow xuất bản](https://img.shields.io/github/actions/workflow/status/DibbayajyotiRoy/fresco/publish.yml?style=flat-square&label=publish)](https://github.com/DibbayajyotiRoy/fresco/actions/workflows/publish.yml)
[![Sao trên GitHub](https://img.shields.io/github/stars/DibbayajyotiRoy/fresco?style=flat-square)](https://github.com/DibbayajyotiRoy/fresco/stargazers)

[Trang web](https://fresco.dibbayajyoti.com) · [Cài đặt](#cài-đặt) · [Môi trường được hỗ trợ](#môi-trường-được-hỗ-trợ) · [So sánh](#so-sánh-fresco-với-wallpaper-engine-hidamari-komorebi-và-mpvpaper) · [FAQ](#câu-hỏi-thường-gặp-faq) · [Widget](#widget-trên-màn-hình-nền-lời-bài-hát-đồng-hồ-hiệu-ứng-hình-ảnh-âm-thanh-và-ảnh-bìa-album) · [Nhật ký thay đổi](CHANGELOG.md) · [Issues](https://github.com/DibbayajyotiRoy/fresco/issues)

<img src="./data/screenshots/gallery.png" alt="Thư viện hình nền của Fresco trên màn hình nền Linux tối màu: lưới hình nền video, ảnh và trình chiếu có thể tìm kiếm, một video được đánh dấu Đang dùng và nhãn trạng thái cho biết đang giải mã phần cứng bằng NVDEC" width="800" />

</div>

## Fresco là gì?

Fresco biến video, GIF, ảnh, trình chiếu hoặc danh sách phát thành hình nền chạy phía sau các cửa sổ của bạn, thông qua một ứng dụng GTK4 / libadwaita không cần dùng terminal. Việc phát được thực hiện qua mpv, nên GPU đảm nhận giải mã và mức dùng CPU gần như ở trạng thái rảnh — chi phí điện năng thực tế xem ở mục [Hiệu năng](#hiệu-năng-và-thời-lượng-pin). Hình nền của bạn được khôi phục mỗi lần đăng nhập.

- **Phát được:** video (mp4, webm, mkv, avi, mov), GIF động, ảnh, trình chiếu ảnh và danh sách phát video.
- **Desktop:** phiên X11 (GNOME trên Xorg, Cinnamon, Xfce, MATE, Deepin 25) và Wayland (COSMIC, Hyprland, Sway, KDE Plasma 6, Cinnamon 6.8+). GNOME trên Wayland chỉ hiện được một khung hình tĩnh — xem [Môi trường được hỗ trợ](#môi-trường-được-hỗ-trợ).
- **Giải mã phần cứng:** VA-API (Intel, AMD) và NVDEC (NVIDIA), thông qua mpv.
- **Thư viện:** liên kết một thư mục hình nền bạn đã có thay vì nhập vào, và giữ đồng bộ khi tệp được thêm vào hay bị xóa đi. Thư viện chỉ vẽ những gì đang hiện trên màn hình, nên hàng nghìn hình nền vẫn nhanh và nhẹ.
- **Widget:** lời bài hát đồng bộ theo nhạc, đồng hồ (kể cả đồng hồ thế giới), hiệu ứng hình ảnh âm thanh và ảnh bìa album, được vẽ thẳng lên hình nền. Tất cả đều tắt theo mặc định.
- **Điều khiển:** ứng dụng, bảng lệnh Ctrl+K, cùng `fresco next` / `fresco prev` / `fresco random` để gán phím tắt.
- **Cài đặt:** gói `.deb` cho Ubuntu, Debian, Linux Mint, Pop!_OS, elementary OS, Kali và Deepin (một dòng lệnh, tải thủ công hoặc qua Cửa hàng ứng dụng Deepin); trên Fedora, Arch và các bản phân phối khác thì biên dịch từ mã nguồn — xem [Cài đặt](#cài-đặt).
- **Giấy phép:** miễn phí và mã nguồn mở theo [GPL-3.0-or-later](LICENSE), không quảng cáo, không cần tài khoản.
- **Người dùng:** 3.000+ người ở 128 quốc gia. Phiên bản mới nhất: 1.1.51.

## Cài đặt

Fresco được đóng gói dạng `.deb` cho các bản phân phối họ Debian và biên dịch từ mã nguồn ở mọi nơi khác.

### Ubuntu, Debian, Linux Mint, Pop!_OS, elementary OS và Kali Linux

Cài đặt bằng một dòng lệnh — tải gói `.deb` mới nhất từ GitHub Releases rồi cài bằng apt:

```bash
curl -fsSL https://github.com/DibbayajyotiRoy/fresco/releases/latest/download/install.sh | FRESCO_SOURCE=github bash
```

Hoặc tải `.deb` từ [Releases](https://github.com/DibbayajyotiRoy/fresco/releases/latest) và chạy:

```bash
sudo apt install ./fresco_*.deb
```

### Deepin 25

Mở **Cửa hàng ứng dụng**, tìm **Fresco** rồi nhấp **Cài đặt**. Fresco đã có trên Cửa hàng ứng dụng cộng đồng deepin, nên không cần tải thủ công và bản cập nhật sẽ đến qua cửa hàng. Cách cài một dòng lệnh ở trên cũng dùng được trên Deepin.

### Fedora, Arch Linux và các bản phân phối khác

Hiện chưa có gói dựng sẵn cho các bản phân phối này, nên hãy biên dịch từ mã nguồn. Bạn cần Rust (cài qua [rustup](https://rustup.rs)) cùng các tệp header phát triển của GTK4, libadwaita và libmpv:

```bash
git clone https://github.com/DibbayajyotiRoy/fresco.git && cd fresco
cargo build --release --all-features
```

Trên các compositor Wayland, Fresco còn cần trình kết xuất `mpvpaper`; gói `.deb` đã đóng kèm sẵn còn bản biên dịch từ mã nguồn thì không:

```bash
# Arch Linux, CachyOS, EndeavourOS (AUR)
yay -S mpvpaper

# Fedora và các bản phân phối khác: tự biên dịch với libmpv của hệ thống
scripts/build-mpvpaper.sh
install -Dm755 target/release/mpvpaper ~/.local/bin/mpvpaper   # phải nằm trong PATH
```

Fresco không có trên AUR hay Flathub — gói AUR tên `fresco` là một chương trình vật lý không liên quan. Chi tiết và cách xử lý sự cố nằm trong [docs/INSTALL.md](docs/INSTALL.md).

Sau khi cài, hãy mở **Fresco** từ trình khởi chạy ứng dụng. Nếu thấy có gì bất thường, hãy chạy `fresco doctor`.

## Cách đặt video làm hình nền trên Linux

Cài Fresco, nhấp **Thêm**, chọn video của bạn rồi nhấp **Đặt làm hình nền**. Video sẽ phát phía sau các cửa sổ và tự động trở lại khi đăng nhập.

1. Cài Fresco (xem phần trên) và mở nó từ trình khởi chạy ứng dụng.
2. Nhấp **Thêm** rồi chọn video, GIF hoặc ảnh — hoặc dán một liên kết bằng **Từ liên kết**.
3. Nếu muốn, hãy cắt hoặc xoay trong trình chỉnh sửa.
4. Nhấp **Đặt làm hình nền**, rồi đóng ứng dụng.

Hình nền vẫn tiếp tục phát sau khi cửa sổ ứng dụng đã đóng.

## Tính năng

- **Mọi loại media** — video lặp (mp4/webm/mkv), GIF động, ảnh tĩnh, trình chiếu ảnh, danh sách phát nhiều video
- **Thêm từ liên kết** — dán một pin Pinterest hoặc URL video/ảnh trực tiếp; Fresco tải về rồi mở trình chỉnh sửa cắt
- **Liên kết thư mục** — dùng một thư mục hình nền bạn đã có (ảnh, GIF và video) mà không cần nhập vào; thư mục luôn đồng bộ khi bạn thêm và xóa tệp, thư mục con là tùy chọn, còn mục yêu thích và các chỉnh sửa của bạn được giữ theo từng tệp
- **Thư viện lớn** — thư viện chỉ vẽ những hình nền gần vị trí cuộn của bạn, nên hàng nghìn hình nền mở trong khoảng hai giây và dùng vài trăm megabyte (trên thư viện thử nghiệm 4.000 hình nền: khoảng 2 giây và 325 MB, so với 9 đến 13 giây và 1,6 GB)
- **Giải mã phần cứng** — giải mã video bằng GPU (VA-API / NVDEC) giữ CPU gần như rảnh bằng cách chuyển công việc sang bộ xử lý video, nơi chi phí rẻ hơn rõ rệt (không phải miễn phí — xem [Hiệu năng](#hiệu-năng-và-thời-lượng-pin))
- **Tiết kiệm điện** — co giãn ảnh trên GPU nhẹ hơn cho laptop; đo được là giảm khoảng một nửa điện năng GPU (xem [Hiệu năng](#hiệu-năng-và-thời-lượng-pin))
- **Nhiều màn hình** — mỗi màn hình một hình nền riêng, và phát đồng bộ khi cùng một video chạy trên nhiều màn hình
- **Lịch ngày và đêm** — đổi hình nền theo bộ hẹn giờ, khung giờ tùy ý hoặc theo bình minh/hoàng hôn
- **Widget trên màn hình nền** — lời bài hát đồng bộ, đồng hồ theo giao diện, đồng hồ thế giới, hiệu ứng hình ảnh âm thanh và đĩa ảnh bìa album xoay tròn, được vẽ thẳng vào hình nền nên không che cửa sổ nào của bạn (đồng hồ thế giới có thêm chế độ tùy chọn nổi trên cửa sổ); tất cả đều tắt theo mặc định (xem [Widget trên màn hình nền](#widget-trên-màn-hình-nền-lời-bài-hát-đồng-hồ-hiệu-ứng-hình-ảnh-âm-thanh-và-ảnh-bìa-album))
- **Màn hình khóa** — hiển thị hình nền và widget (đồng hồ, lời chào, bài đang phát, pin và hơn thế nữa) trên màn hình khóa thật thay cho màn hình khóa mặc định — video động và widget trên COSMIC 1.9+ và KDE Plasma 6, hình nền kèm widget qua `fresco lock` trên Sway/Hyprland/niri và X11, còn lại là một khung hình tĩnh. Tắt theo mặc định, và Fresco không bao giờ nhìn thấy mật khẩu của bạn — xem [docs/LOCKSCREEN.md](docs/LOCKSCREEN.md)
- **Điều khiển bằng bàn phím** — `fresco next`, `fresco prev` và `fresco random` để gán phím tắt cho compositor (cửa sổ Fresco đang mở sẽ nhận thay đổi thay vì hoàn nguyên nó), cùng bảng lệnh Ctrl+K ngay trong ứng dụng
- **Quản lý hàng loạt** — chọn nhiều hình nền cùng lúc và xóa chỉ trong một bước
- **Danh mục tích hợp** — duyệt ngay trong ứng dụng các hình nền tuyển chọn, có bản quyền hợp lệ
- **Tự động tạm dừng khi toàn màn hình** — theo từng màn hình, trên X11, các compositor wlroots, COSMIC và KDE Plasma (qua plugin hình nền của nó); tùy chọn áp dụng cả cho cửa sổ phóng to tối đa, cộng thêm tạm dừng khi dùng pin
- **Tiện ích tab mới cho trình duyệt** — phản chiếu hình nền của bạn trên mọi tab mới (Chrome/Brave/Edge/Firefox; nạp dạng chưa đóng gói từ [`./extension`](extension))
- **Hỗ trợ Deepin DDE** — trên Deepin 25, Fresco tự điều chỉnh cho phù hợp với desktop DDE, và nhấp vào màn hình nền sẽ đưa các biểu tượng trở lại trong mười giây bất cứ khi nào bạn cần (xem [Khắc phục sự cố](#biểu-tượng-màn-hình-nền-bị-ẩn-khi-hình-nền-chạy-trên-deepin))
- **Trình chỉnh sửa cắt và xoay**, âm thanh/âm lượng riêng cho từng hình nền, hiệu ứng chuyển cho trình chiếu và thư viện có thể tìm kiếm

## Môi trường được hỗ trợ

| Môi trường | Hình nền động | Ghi chú |
|---|---|---|
| X11 (GNOME, Cinnamon, XFCE, …) | ✅ | Trình kết xuất nhúng. GNOME chỉ dùng được khi bản phân phối còn cung cấp phiên Xorg (xem ghi chú GNOME bên dưới) |
| Kali Linux (Xfce, X11) | ✅ | Chạy trên phiên mặc định của Kali là Xfce trên X11, với cùng trình kết xuất nhúng như các desktop X11 khác |
| Xfce (X11) | ✅ | Biểu tượng màn hình nền vẫn hiển thị phía trên hình nền, và bạn có thể nhấp, chọn và kéo chúng |
| MATE (X11) | ✅ | Biểu tượng màn hình nền vẫn hiển thị và nhấp được phía trên hình nền |
| Deepin 25 (DDE, X11) | ✅ | Tự động thích ứng với DDE — đã được cộng đồng kiểm chứng trên Deepin 25 Community build1 |
| COSMIC (Wayland) | ✅ | layer-shell |
| Hyprland | ✅ | layer-shell |
| Sway | ✅ | layer-shell |
| Cinnamon 6.8 trở lên (Wayland) | ✅ | Compositor của Cinnamon có layer-shell từ bản 6.8. Cinnamon 6.6 trở xuống không có layer-shell và chỉ giữ một khung hình tĩnh trên Wayland |
| KDE Plasma 6 (X11 và Wayland) | ✅ | Được đặt thông qua plugin hình nền của chính plasmashell, nên biểu tượng màn hình nền vẫn hiển thị. Cần `qml6-module-qtmultimedia` để phát video. Việc phát do Qt đảm nhận chứ không phải mpv: tắt tiếng, không tinh chỉnh hwdec, không cắt hay hiệu ứng chuyển; danh sách phát chỉ phát tệp đầu tiên và trình chiếu chỉ hiện khung hình đầu tiên. Tùy chọn Cách hiển thị (Lấp đầy, Vừa khung, Kéo giãn) vẫn được áp dụng |
| GNOME trên Wayland | ⚠️ | Chỉ một khung hình tĩnh — Mutter không cung cấp bề mặt cho hình nền động, nên không có video và không có widget. Muốn có video động cần một tiện ích mở rộng GNOME của Fresco (đang được lên kế hoạch) |

Mỗi môi trường ở trên đều được chạy kiểm thử headless trong CI ở mỗi bản phát hành.

**Ghi chú về GNOME.** GNOME trên Wayland là desktop phổ biến duy nhất mà Fresco chỉ có thể hiện một khung hình tĩnh. Cách lách cũ, tức đăng nhập vào phiên Xorg, đang dần biến mất: GNOME 49 đã vô hiệu hóa phiên X11 và GNOME 50 đã loại bỏ hẳn, nên Ubuntu 25.10 trở lên (kể cả 26.04 LTS) và Fedora 43 trở lên không còn phiên Xorg nào để chuyển sang. Ubuntu 22.04 và 24.04 vẫn cung cấp "Ubuntu on Xorg". Ở nơi không có phiên Xorg, video động trên desktop GNOME gốc phải chờ tiện ích mở rộng GNOME của Fresco đang được lên kế hoạch; Fresco nói rõ điều này ngay trong ứng dụng thay vì giả vờ video đang phát. KDE Plasma, COSMIC, Hyprland, Sway và mọi desktop X11 đều không bị ảnh hưởng.

> "Dễ dùng với giao diện gọn gàng — một trong số ít ứng dụng hình nền động được điều chỉnh đúng cách cho Deepin 25, cài được bằng .deb và chạy mượt với phát video tăng tốc phần cứng."
>
> — 柒玖 (diễn đàn deepin) / 柒仈玖 (GitHub), thử nghiệm trên Deepin 25 Community build1, phiên X11, Intel Alder Lake-N [Intel Graphics]

Deepin 25 dùng X11 làm phiên mặc định, và đó là phiên mà Fresco được kiểm chứng ở đó. Compositor Wayland riêng của Deepin, [Treeland](https://github.com/linuxdeepin/treeland), vẫn đang được phát triển, nên Fresco chưa đưa ra tuyên bố nào về Deepin trên Wayland.

## So sánh Fresco với Wallpaper Engine, Hidamari, Komorebi và mpvpaper

| Công cụ | Nền tảng | Wayland | X11 | Giao diện | Phát được | Giấy phép |
|---|---|---|---|---|---|---|
| **Fresco** | Linux | ✅ compositor layer-shell và KDE Plasma 6; khung hình tĩnh trên GNOME | ✅ | Ứng dụng GTK4, kèm CLI `fresco` | Video, GIF, ảnh, trình chiếu, danh sách phát | GPL-3.0-or-later |
| **Wallpaper Engine** | Windows (Steam); không có bản Linux chính thức | n/a | n/a | Ứng dụng đồ họa | Hình nền video, scene và web | Trả phí, mã nguồn đóng |
| **[linux-wallpaperengine](https://github.com/Almamu/linux-wallpaperengine)** | Linux; cần tài nguyên từ bản Wallpaper Engine mua trên Steam | ✅ compositor có layer-shell | ⚠️ không dùng được khi môi trường desktop (GNOME, KDE) tự vẽ nền | Dòng lệnh; README của nó liệt kê các GUI riêng do cộng đồng làm | Hình nền của Wallpaper Engine | GPL-3.0 |
| **[Hidamari](https://github.com/jeffshee/hidamari)** | Linux, tập trung vào GNOME | ⚠️ GNOME Wayland | ✅ | Ứng dụng đồ họa | Video | GPL-3.0 |
| **[Komorebi](https://github.com/cheesecakeufo/komorebi)** | Linux | — | — | Ứng dụng đồ họa | Hình nền ảnh, video và trang web | GPL-3.0 |
| **[mpvpaper](https://github.com/GhostNaN/mpvpaper)** | Linux, compositor nền wlroots | ✅ layer-shell | ❌ | Dòng lệnh | Video, thông qua mpv | GPL-3.0 |
| **xwinwrap** | Linux, chỉ X11 | ❌ | ✅ | Dòng lệnh | Bất cứ trình phát nào bạn gắn vào (mpv, gifview) | — |
| **swww** (nay là [awww](https://codeberg.org/LGFae/awww)) | Linux, compositor layer-shell; không phải GNOME | ✅ | ❌ | Daemon dòng lệnh | GIF động và ảnh, không phát video | GPL-3.0 |

Fresco đóng kèm `mpvpaper` làm trình kết xuất cho Wayland, nên nó xây dựng trên dự án đó chứ không cạnh tranh với nó. Dấu `—` nghĩa là tài liệu của chính dự án không nêu rõ. Đã đối chiếu với kho mã công khai của từng dự án vào 2026-10-16: lần push gần nhất của Komorebi vào kho mã là tháng 7 năm 2025, còn kho GitHub của swww đã được lưu trữ (archived) vì dự án chuyển sang Codeberg với tên awww.

## Hiệu năng và thời lượng pin

柒玖 (diễn đàn deepin) / 柒仈玖 (GitHub) đã đo công suất package bằng `turbostat` trên Intel N150 (Deepin 25, VA-API, mỗi mức chạy hai lần) trong khi một hình nền video đang phát:

| Video | Tiết kiệm điện | Công suất GPU | Tổng công suất package |
|---|---|---|---|
| 1080p 60fps | Chất lượng đầy đủ | 1,37 W | 6,00 W |
| 1080p 60fps | **Thấp** (mặc định) | **0,63 W** (−54%) | **4,03 W** (−33%) |
| 4K 60fps | Chất lượng đầy đủ | 2,77 W | 7,94 W |
| 4K 60fps | **Thấp** (mặc định) | **1,60 W** (−42%) | **5,95 W** (−25%) |
| 4K 60fps | Chất lượng thấp nhất | 0,99 W (−65%) | 4,97 W (−37%) |

Tiết kiệm điện làm giảm chi phí co giãn ảnh trên GPU cho từng khung hình. Không có khung hình nào bị bỏ và giải mã phần cứng không bị đụng đến, nên việc phát vẫn mượt — cái phải đánh đổi là độ sắc nét của ảnh, không phải chuyển động. Thấp là mặc định; với nguồn 4K, nên cân nhắc chọn chất lượng thấp nhất.

**Cách đọc các con số này.** Giải mã phần cứng không biến hình nền động thành miễn phí — nó chuyển công việc từ CPU sang bộ xử lý video, nơi rẻ hơn chứ không biến mất. Chỉ nói "mức dùng CPU thấp" là một lập luận yếu, vì thứ được đem ra so sánh là giải mã bằng phần mềm, vốn tệ hơn nhiều; thước đo trung thực là mức tiêu thụ của cả hệ thống, vì vậy bảng báo cáo **tổng công suất package** chứ không chỉ công suất GPU.

Hai lưu ý về các số liệu trên, để không đọc chúng nhiều hơn những gì chúng thực sự nói:

- Đó là **tổng công suất package khi một hình nền đang phát**, không phải chi phí tăng thêm của riêng hình nền. Không có mốc so sánh lúc rảnh được ghi trên cùng máy, nên chênh lệch giữa các con số này và một desktop đang rảnh chưa được xác lập ở đây. Các tỷ lệ phần trăm so sánh các *mức* tiết kiệm điện với nhau, và đó là điều chúng có giá trị.
- Đó là số liệu của một máy duy nhất (Intel N150, Alder Lake-N, VA-API, Deepin 25, mỗi mức chạy hai lần). GPU rời, NVDEC và các trình điều khiển khác sẽ cho kết quả khác.

Hình nền động luôn tốn hơn hình nền tĩnh. Tiết kiệm điện, tự động tạm dừng khi toàn màn hình và Tạm dừng khi dùng pin sinh ra là để giới hạn chi phí đó, chứ không phải để giả vờ là nó bằng không.

## Câu hỏi thường gặp (FAQ)

### Có ứng dụng thay thế Wallpaper Engine trên Linux không?

Có. Fresco là ứng dụng Linux miễn phí, mã nguồn mở, phát hình nền video, GIF và ảnh của chính bạn qua giao diện đồ họa, với giải mã phần cứng và không cần lớp tương thích nào. Nó không mở được các tệp hình nền của chính Wallpaper Engine; nếu bạn đã mua Wallpaper Engine và muốn dùng chúng trên Linux, dự án riêng [linux-wallpaperengine](https://github.com/Almamu/linux-wallpaperengine) sẽ chạy chúng từ dòng lệnh và cần tài nguyên từ bản Wallpaper Engine mua trên Steam. Xem [bảng so sánh](#so-sánh-fresco-với-wallpaper-engine-hidamari-komorebi-và-mpvpaper).

### Wallpaper Engine có chạy được trên Linux không?

Không chạy native. Wallpaper Engine là ứng dụng Windows và không có bản Linux chính thức. Fresco là lựa chọn thay thế native cho Linux, cài dưới dạng `.deb` và không cần lớp tương thích nào.

### Làm sao đặt video làm hình nền trên Ubuntu?

Cài Fresco, mở nó, nhấp **Thêm**, chọn video của bạn rồi nhấp **Đặt làm hình nền**. Video phát làm nền màn hình nền và được khôi phục khi đăng nhập. Phiên GNOME trên Wayland mặc định của Ubuntu chỉ hiện được một khung hình tĩnh. Trên Ubuntu 22.04 và 24.04, bạn có thể đăng nhập vào phiên **Ubuntu on Xorg** để phát động đầy đủ; Ubuntu 25.10 trở lên (kể cả 26.04 LTS) không còn phiên Xorg, nên ở đó video động trên desktop GNOME gốc cần một tiện ích mở rộng GNOME của Fresco đang được lên kế hoạch. Các bản Ubuntu dùng KDE Plasma, Xfce hoặc MATE không bị ảnh hưởng.

### Làm sao cài hình nền động trên Ubuntu, Linux Mint, Pop!_OS, Fedora hoặc Arch Linux?

Cài gói `.deb` của Fresco trên Ubuntu, Linux Mint và Pop!_OS, hoặc biên dịch từ mã nguồn trên Fedora và Arch Linux. Lệnh cho từng trường hợp có ở mục [Cài đặt](#cài-đặt).

- **Ubuntu:** `.deb`. Trên desktop GNOME mặc định, muốn phát động cần phiên Xorg (Ubuntu 22.04 và 24.04); các bản KDE Plasma, Xfce và MATE không bị ảnh hưởng — xem câu trả lời về Ubuntu ở trên.
- **Linux Mint:** `.deb`. Hỗ trợ Cinnamon trên X11, và Cinnamon trên Wayland từ bản 6.8.
- **Pop!_OS:** `.deb`. Hỗ trợ desktop COSMIC (Wayland layer-shell).
- **Fedora:** biên dịch từ mã nguồn. GNOME của Fedora 43 trở lên chỉ có Wayland nên hiện khung hình tĩnh; hỗ trợ KDE Plasma và các desktop X11.
- **Arch Linux:** biên dịch từ mã nguồn và cài `mpvpaper` từ AUR (`yay -S mpvpaper`).

Chỉ cách cài bằng `.deb` được CI của Fresco kiểm thử.

### Fresco hỗ trợ những bản phân phối Linux nào?

Fresco cung cấp gói `.deb` cho các bản phân phối dựa trên Debian và Ubuntu: Ubuntu, Pop!_OS, Linux Mint, Debian, elementary OS, Deepin 25 và Kali Linux. Các bản phân phối khác có thể biên dịch từ mã nguồn — xem [Cài đặt](#cài-đặt) và [docs/INSTALL.md](docs/INSTALL.md).

### Hình nền động có tốn nhiều CPU, GPU hoặc pin không?

CPU thì không. Pin thì có một chút — hình nền động không bao giờ miễn phí.

Fresco giải mã video trên GPU (VA-API / NVDEC), nên mức dùng CPU gần như rảnh. Điều đó chuyển chi phí sang bộ xử lý video chứ không xóa bỏ nó. Đo trên Intel N150 ở mức Tiết kiệm điện mặc định, hình nền 1080p tiêu thụ 0,63 W công suất GPU với tổng công suất package là 4,03 W khi đang phát (xem [Hiệu năng](#hiệu-năng-và-thời-lượng-pin) để biết cách đọc, kể cả những gì nó *không* nói lên). Tự động tạm dừng khi toàn màn hình và Tạm dừng khi dùng pin giúp chi phí đó không ngốn pin vào lúc quan trọng nhất.

### Fresco có chạy trên Wayland không?

Có, trên các compositor triển khai giao thức layer-shell — COSMIC, Hyprland và Sway — và trên KDE Plasma 6 (X11 và Wayland) thông qua plugin hình nền của chính plasmashell. GNOME trên Wayland là ngoại lệ: Mutter không cung cấp bề mặt hình nền, nên Fresco hiện một khung hình tĩnh ở đó và nói rõ điều này trong ứng dụng. Các phiên X11 được hỗ trợ đầy đủ.

### Fresco có chạy trên GNOME không?

Có trên X11, với đầy đủ hình nền động — video, widget, mọi thứ. Trên **GNOME dưới Wayland**, Mutter không cung cấp bề mặt hình nền để bất kỳ ứng dụng nào vẽ vào, nên Fresco hiện một khung hình tĩnh ở đó và widget không dùng được. Nếu bản phân phối của bạn vẫn còn phiên Xorg (Ubuntu 22.04 và 24.04 có), hãy đăng xuất và chọn "GNOME on Xorg" hoặc "Ubuntu on Xorg" để phát động đầy đủ. GNOME 49 đã vô hiệu hóa phiên đó và GNOME 50 đã loại bỏ nó, nên Ubuntu 25.10 trở lên (kể cả 26.04 LTS) và Fedora 43 trở lên không có: ở đó, video động trên GNOME cần một tiện ích mở rộng GNOME của Fresco đang được lên kế hoạch. `fresco doctor` cho bạn biết mình đang ở trường hợp nào.

### Fresco có chạy trên KDE Plasma, COSMIC, Hyprland hoặc Sway không?

Có, trên cả bốn. COSMIC, Hyprland và Sway phát video động qua layer-shell. KDE Plasma 6 (X11 và Wayland) dùng plugin hình nền riêng của Fresco bên trong plasmashell, giúp biểu tượng màn hình nền vẫn hiển thị nhưng phát qua Qt thay vì mpv: tắt tiếng, không cắt hay hiệu ứng chuyển, và cần `qml6-module-qtmultimedia` để phát video. Tùy chọn Cách hiển thị (Lấp đầy, Vừa khung hoặc Kéo giãn) cũng áp dụng ở đó.

### Fresco có chạy trên Linux Mint, Xfce, Cinnamon hoặc MATE không?

Có. Cinnamon, Xfce và MATE trên X11 dùng trình kết xuất nhúng, và biểu tượng màn hình nền vẫn hiển thị phía trên hình nền trên Xfce và MATE; trên Xfce, bạn có thể nhấp, chọn và kéo chúng như thường. Cinnamon trên Wayland chạy được từ Cinnamon 6.8, bản đã thêm layer-shell.

### Làm sao đổi hình nền bằng phím tắt?

Gán `fresco next`, `fresco prev` hoặc `fresco random` cho một phím trong cài đặt compositor hoặc desktop của bạn — chúng đổi hình nền mà không cần mở ứng dụng (`random` không bao giờ chọn hình nền đang phát). Chúng lần lượt đi qua thư viện của bạn theo đúng thứ tự ứng dụng hiển thị, hoặc chỉ trong thư mục bạn đang mở ở đó, và sẽ khởi động dịch vụ Fresco nếu nó chưa chạy. Nếu cửa sổ Fresco đang mở vào lúc đó, nó sẽ nhận hình nền mới thay vì đặt lại hình nền cũ. Trong ứng dụng, Ctrl+K mở bảng lệnh. Ví dụ trên Sway:

```
bindsym $mod+Right  exec fresco next
bindsym $mod+Left   exec fresco prev
bindsym $mod+Return exec fresco random
```

### Có thể dùng hình nền khác nhau cho từng màn hình không?

Có. Fresco hỗ trợ hình nền riêng cho từng màn hình, và khi cùng một video được dùng trên nhiều màn hình thì việc phát được giữ đồng bộ.

### Fresco có hỗ trợ GIF và trình chiếu ảnh không?

Có — GIF động, ảnh tĩnh, trình chiếu ảnh có hiệu ứng chuyển (mờ dần qua màn hình đen, Ken Burns, phóng to và làm mờ) và danh sách phát nhiều video, bên cạnh các tệp video.

### Fresco có dùng được thư mục hình nền sẵn có mà không cần nhập vào không?

Có. Chọn **Liên kết thư mục…** trong menu (hoặc **Liên kết thư mục này** sau khi chọn **Thêm thư mục**) để chỉ cho Fresco một thư mục bạn vốn đã lưu hình nền. Ảnh, GIF và video trong đó hiện trong thư viện của bạn ở một mục riêng, ngay tại chỗ, không cần nhập vào, và thư viện luôn đồng bộ khi bạn thêm và xóa tệp trong thư mục đó. **Bao gồm cả thư mục con** là tùy chọn (sâu tối đa bốn cấp), còn mục yêu thích và các chỉnh sửa của bạn được ghi nhớ theo từng tệp. Nếu thư mục biến mất, chẳng hạn khi ổ đĩa bị rút ra, mục của nó vẫn được đánh dấu “(không khả dụng)” và tự phục hồi. **Hủy liên kết thư mục**, trong menu của mục, gỡ mục đó mà không xóa tệp nào. **Thêm** vẫn nhập từng tệp riêng lẻ như trước.

### Fresco có xử lý được hàng nghìn hình nền không?

Có. Thư viện chỉ dựng thẻ cho những hình nền gần chỗ bạn đang cuộn, thay vì dựng tất cả cùng lúc, nên một bộ sưu tập lớn không làm ứng dụng nặng nề. Trên một thư viện thử nghiệm 4.000 hình nền (một nửa là ảnh, một nửa là video), thư viện xuất hiện sau khoảng 2 giây thay vì 9 đến 13 giây, và ứng dụng dùng khoảng 325 MB bộ nhớ thay vì 1,6 GB. Các con số này đến từ một bản dựng phát triển chạy dưới tải trên một máy, nên hãy xem chúng là mốc tham khảo chứ không phải lời hứa.

### Có thể dùng video YouTube làm hình nền không?

Không thể bằng cách dán liên kết của nó. **Từ liên kết** chấp nhận pin Pinterest và liên kết trực tiếp tới tệp video hoặc ảnh (mp4, webm, mkv, mov, gif, jpg, png, webp); các liên kết khác bị từ chối kèm một thông báo. Hãy thêm tệp video bạn đã có bằng **Thêm**, hoặc dán liên kết trực tiếp tới tệp.

### Fresco có miễn phí và mã nguồn mở không?

Có. Fresco miễn phí và mã nguồn mở theo GPL-3.0-or-later. Không quảng cáo, không tài khoản và không có gói trả phí.

### Fresco có thu thập dữ liệu của tôi không?

Không có gì được gửi đi cho đến khi bạn trả lời hộp thoại đồng ý một lần. Bạn chọn giữa một lượt đếm ẩn danh hằng ngày (mã cài đặt ngẫu nhiên, quốc gia, phiên bản ứng dụng) và lượt đếm đó cộng thêm thông tin chi tiết tùy chọn; đặt `telemetry = false` trong `~/.config/fresco/config.toml` thì không gửi gì cả. Xem [Quyền riêng tư và điều khoản sử dụng](#quyền-riêng-tư-và-điều-khoản-sử-dụng) để biết từng trường dữ liệu.

### Fresco khác mpvpaper ở điểm nào?

`mpvpaper` là lớp bọc mpv dùng dòng lệnh, phát một video làm nền layer-shell trên Wayland — không có GUI, không có thư viện, không hỗ trợ X11. Fresco là ứng dụng desktop GTK4 được xây quanh nó: có thư viện hình nền, trình chỉnh sửa cắt/xoay, lập lịch, đồng bộ nhiều màn hình, widget trên màn hình nền và hỗ trợ X11, không cần dùng terminal. Fresco đóng kèm `mpvpaper` làm trình kết xuất cho Wayland chứ không thay thế nó — xem [bảng so sánh](#so-sánh-fresco-với-wallpaper-engine-hidamari-komorebi-và-mpvpaper).

### Có dùng Fresco trên Kali Linux được không?

Được. Kali Linux dựa trên Debian, nên Fresco cài theo cách giống hệt trên Debian hoặc Ubuntu — bằng gói `.deb` hoặc tập lệnh cài đặt một dòng (xem [Cài đặt](#cài-đặt)). Desktop mặc định của Kali, Xfce trên X11, được hỗ trợ thông qua trình kết xuất X11 nhúng của Fresco.

### Làm sao xóa nhiều hình nền cùng lúc?

Nhấp **Chọn** ở thanh chân (hoặc nhấp chuột phải vào một hình nền rồi chọn **Chọn…**), đánh dấu những hình bạn muốn, rồi nhấp **Xóa**. **Chọn tất cả** tuân theo kết quả tìm kiếm hiện tại, nên bạn có thể tìm trước rồi xóa cả loạt. Xóa một hình nền chỉ gỡ nó khỏi thư viện Fresco — tệp gốc trên ổ đĩa vẫn được giữ lại.

### Làm sao gỡ cài đặt Fresco?

Nếu bạn đã cài bằng `.deb` hoặc dùng tập lệnh cài đặt, hãy chạy `sudo apt remove fresco`. Thư viện và cấu hình của bạn nằm trong `~/.local/share/fresco/` và `~/.config/fresco/`; xóa các thư mục đó để loại bỏ mọi dấu vết. Để lấy lại hình nền gốc của desktop mà không cần gỡ cài đặt, hãy mở Fresco và nhấp **Dừng hình nền**.

## Widget trên màn hình nền: lời bài hát, đồng hồ, hiệu ứng hình ảnh âm thanh và ảnh bìa album

Fresco vẽ năm widget tùy chọn lên hình nền của bạn. Tất cả đều **tắt theo mặc định**; bật chúng từ menu ứng dụng (Ctrl+,) → **Nâng cao…**.

| Widget | Bạn nhận được gì |
|---|---|
| **Lời bài hát** | Dòng đang hát, khớp theo nhạc. Bốn bộ cài sẵn (Tối giản, Karaoke, Phụ đề, Thẻ), lưới vị trí chín điểm, thanh trượt độ lệch đồng bộ, dòng kế tiếp được làm mờ (tùy chọn), cùng tên bài hát và nghệ sĩ (tùy chọn) |
| **Đồng hồ** | Tám giao diện — Điện tử, Tối giản, Điện tử số, Xếp chồng, Nhiều chữ, Thẻ, NOS và mặt đồng hồ Màn hình khóa kiểu điện thoại. Định dạng 12 hoặc 24 giờ, ngày tháng tùy chọn, điều chỉnh màu sắc và độ mờ đục. Giây tắt theo mặc định vì chúng tốn gấp 60 lần số lần vẽ lại |
| **Đồng hồ thế giới** | Giờ ở nhiều thành phố cùng lúc, chọn từ danh sách đầy đủ các múi giờ, dạng văn bản 12 giờ thuần (ví dụ `Tokyo  9:41 PM`) với nhãn tùy chọn cho từng thành phố. Vẽ lại mỗi phút một lần. Mới trong 1.1.50 |
| **Hiệu ứng hình ảnh âm thanh** | Năm kiểu — Cột, Lật gương, Dạng sóng, Chấm, Vòng tròn — với bộ chọn màu, pha hai màu hoặc cầu vồng |
| **Ảnh bìa album** | Ảnh bìa của bài đang phát trên một chiếc đĩa than xoay; đĩa ngừng xoay khi tạm dừng phát |

Ngoại trừ chế độ tùy chọn **Nổi trên cửa sổ** của đồng hồ thế giới, các widget được vẽ vào hình nền thông qua lớp OSD của mpv chứ không vào một cửa sổ riêng, nên chúng không bao giờ nằm trên cửa sổ của bạn, không bao giờ chặn cú nhấp chuột và hoạt động giống hệt nhau trên X11 và mọi compositor layer-shell. Khi đang phát nhạc và bật các widget lời bài hát, đồng hồ, hiệu ứng hình ảnh âm thanh và ảnh bìa album, chi phí đo được là **0,8% của một nhân CPU** — hầu hết là do thu âm thanh, vì không có gì được vẽ lại trừ khi nội dung của nó thay đổi. Các widget cũng tự trở lại sau khi máy tạm ngưng rồi hoạt động trở lại.

Theo mặc định, widget xuất hiện trên **mọi màn hình**. Để giữ chúng chỉ ở một màn hình, hãy thêm `monitor = "DP-1"` vào khối `[widgets]` của `config.toml` (hiện chưa có điều khiển GUI cho việc này). Widget **không dùng được trên GNOME dưới Wayland**, nơi không có bề mặt hình nền động để Fresco vẽ vào — cùng lý do khiến hình nền ở đó chuyển sang khung hình tĩnh.

### Có thể thêm đồng hồ vào hình nền màn hình nền Linux không?

Có. Bật widget **Đồng hồ** để có một chiếc đồng hồ theo giao diện được vẽ lên hình nền, hoặc **Đồng hồ thế giới** để hiển thị nhiều thành phố cùng lúc. Đồng hồ thế giới nằm trên hình nền như các widget khác; chế độ tùy chọn **Nổi trên cửa sổ** nâng nó lên trên mọi ứng dụng với thao tác nhấp chuột xuyên qua, hoạt động trên X11 có compositor và trên các compositor layer-shell (KDE Plasma, COSMIC, Sway, Hyprland), và quay về dạng nằm trên hình nền ở những phiên không thể cho nó nổi lên, như GNOME trên Wayland.

### Có thể hiển thị lời bài hát trên màn hình nền Linux không?

Có. Fresco vẽ **lời bài hát đồng bộ theo thời gian** lên hình nền, theo dõi bất cứ thứ gì đang phát trên hệ thống của bạn qua MPRIS — trình duyệt, ứng dụng nghe nhạc, trình phát video. Chọn bộ cài sẵn, vị trí và độ lệch đồng bộ trong cài đặt của widget Lời bài hát (xem bảng ở trên).

### Linux có widget màn hình nền như Conky không?

Có, và Fresco bổ sung năm widget không cần thanh panel, không cần tiện ích mở rộng và không cần desktop của bạn hỗ trợ: đồng hồ, đồng hồ thế giới, lời bài hát đồng bộ, hiệu ứng hình ảnh âm thanh và đĩa ảnh bìa album xoay tròn. Vì chúng được vẽ vào hình nền thay vì một cửa sổ, chúng hoạt động trên những desktop không có lớp widget riêng — kể cả COSMIC, Hyprland và Sway. Khác với Conky, Fresco **không có widget giám sát hệ thống** (không hiển thị CPU, RAM, nhiệt độ hay mạng), nên nó đứng cạnh Conky như một người bạn đồng hành về nhạc và thời gian chứ không thay thế Conky. GNOME trên Wayland là nơi duy nhất nó không chạy được.

### Có thể hiển thị hiệu ứng hình ảnh theo nhạc trên hình nền màn hình nền không?

Có. Hiệu ứng hình ảnh âm thanh của Fresco phản ứng với bất cứ thứ gì hệ thống đang phát, theo một trong năm kiểu — Cột, Lật gương, Dạng sóng, Chấm hoặc Vòng tròn — với bộ chọn màu, pha hai màu hoặc cầu vồng. Nó tắt theo mặc định và hỏi sự đồng ý trong lần bật đầu tiên, vì nó phải lắng nghe đầu ra âm thanh của bạn; sự đồng ý đó cũng được kiểm tra khi nạp cấu hình, nên tự sửa tay `config.toml` cũng không thể âm thầm bật nó lên. Kết hợp nó với widget ảnh bìa album để có một chiếc đĩa than xoay hiển thị ảnh bìa của bài đang phát.

### Trình phát nhạc nào dùng được với widget lời bài hát?

Bất kỳ trình phát nào công bố siêu dữ liệu MPRIS chuẩn, nhưng độ tin cậy không đồng đều:

| Trình phát | Dùng được? | Ghi chú |
|---|---|---|
| **Firefox** | ✅ | Lựa chọn đáng tin cậy nhất, và là thứ tính năng này được kiểm chứng trên đó |
| Chrome / Brave / Edge / Vivaldi / Opera | ⚠️ | Xem bên dưới |
| Spotify — **trên trình duyệt** | ✅ | Báo cáo đúng vị trí phát |
| Spotify — **ứng dụng Linux native** | ⚠️ | Luôn báo vị trí bằng 0, nên lời bài hát không thể giữ đồng bộ |
| Trình phát cục bộ (VLC, mpv, Rhythmbox, …) | ✅ | Cũng là trường hợp có nhiều khả năng tồn tại tệp `.lrc` cục bộ nhất |

Các trình duyệt họ Chromium giành một tên MPRIS ngay lần đầu có tab nào phát
media và **không bao giờ nhả ra** — một lỗi tồn tại lâu năm của Chromium. Khi
việc phát kết thúc, tiêu đề được xóa nhưng một phiên "ma" cũ vẫn nằm lại trên
bus, đôi khi còn mang theo ảnh bìa. Fresco bỏ qua mọi trình phát không công bố
tiêu đề bài hát, nhờ đó bỏ qua các phiên này, nhưng cách Chromium tự báo cáo
vẫn thiếu nhất quán đến mức Firefox mới là trình duyệt nên dùng cho việc này.

Ứng dụng Spotify native trên Linux trả về `Position: 0` và chưa từng phát sự
kiện seek nào kể từ năm 2018, trên cả bản native lẫn bản snap. Fresco nhận ra
điều đó qua hành vi — ba lần đọc bằng 0 cách nhau khi đang phát — và cho đồng
hồ lời bài hát chạy tự do kể từ lúc đổi bài, nên sẽ lệch nếu bạn tua đi tua
lại. Spotify trên trình duyệt không gặp vấn đề này.

### Lời bài hát lấy từ đâu?

Trước hết là tệp `.lrc` cục bộ: tệp nằm cạnh tệp âm thanh, hoặc tệp khớp trong
thư mục lời bài hát mà bạn trỏ Fresco tới. Đường này chạy được khi ngoại tuyến
và khớp nhất, vì đó là tệp do chính bạn chọn.

Khi không có tệp cục bộ — thường là trường hợp của bạn nếu nghe nhạc trực
tuyến — Fresco tra bài hát trên [LRCLIB](https://lrclib.net), một cơ sở dữ liệu
lời bài hát đồng bộ miễn phí do cộng đồng vận hành, và lưu đệm câu trả lời tại
`~/.cache/fresco/lyrics` để cùng một bài không bao giờ bị tải hai lần. Lần tra
cứu đó gửi tên bài hát, nghệ sĩ và album tới LRCLIB; không có gì được gửi trừ
khi widget lời bài hát đang bật và bài hát không có tệp cục bộ. Fresco không
lưu trữ, không sở hữu và không cấp phép nội dung lời bài hát — các mục trên
LRCLIB do người dùng của nó đóng góp và LRCLIB không nêu giấy phép nào cho
chúng, nên Fresco tải theo yêu cầu và lưu đệm riêng cho từng người dùng thay vì
đi kèm một cơ sở dữ liệu lời bài hát của riêng mình.

Mốc thời gian trong các tệp `.lrc` do con người làm bằng tay nên thường lệch
một chút. Vì thế trong cài đặt lời bài hát có thanh trượt độ lệch đồng bộ.

## Khắc phục sự cố

Hãy chạy `fresco doctor` trước: nó báo cáo phiên của bạn, trình kết xuất nào Fresco đã chọn và lấy từ đâu, cũng như cách lách Xorg cho GNOME có áp dụng được hay không. [docs/INSTALL.md](docs/INSTALL.md) nói về nhiều trường hợp hơn, kể cả mức dùng CPU cao. Hãy đính kèm kết quả của `fresco doctor` khi bạn tạo issue.

Vẫn chưa xử lý được? [Báo cáo lỗi](https://github.com/DibbayajyotiRoy/Fresco/issues/new?template=bug_report.yml) hoặc [đề xuất tính năng](https://github.com/DibbayajyotiRoy/Fresco/issues/new?template=feature_request.yml).

### Fresco không xuất hiện trong trình khởi chạy Deepin sau khi cài — vì sao?

Đây là lỗi đã biết của dde-launchpad: bước làm mới sau khi cài đặt của nó không
nhận ra Fresco. Hãy chạy `killall dde-shell` (nó tự khởi động lại) hoặc đăng
xuất rồi đăng nhập lại, mục này sẽ xuất hiện vĩnh viễn. Bản thân Fresco được
cài đúng — nó có trong danh sách của trình quản lý ứng dụng riêng của Deepin, và
biểu tượng của nó được tìm thấy trong mọi giao diện biểu tượng đã cài. Nếu việc
này xảy ra ở mỗi lần cài lại trên một máy từng có Fresco, hãy chạy
`sh scripts/dde-launcher-diag.sh` — nó chỉ đọc dữ liệu và in ra một bản ghi
đáng để đính kèm vào issue.

### Biểu tượng màn hình nền bị ẩn khi hình nền chạy trên Deepin

Deepin 25 vẽ hình nền và các biểu tượng màn hình nền vào cùng một cửa sổ đục,
nên hình nền động mà bạn nhìn thấy là một cửa sổ xếp phía trên cửa sổ đó —
không có lớp nào nằm giữa chúng. **Nhấp vào màn hình nền và các biểu tượng sẽ
trở lại trong mười giây**, đủ lâu để mở thứ bạn cần; sau đó hình nền quay lại,
và một cú nhấp nữa sẽ được thêm mười giây. Để đổi khoảng thời gian này, đặt
`dde_icon_peek_secs` trong `~/.config/fresco/config.toml` (`0` giữ hình nền
luôn ở trên cùng). Hình nền riêng của Deepin vẫn hiện bất cứ khi nào Fresco tắt
hoặc đang tạm dừng.

### Vì sao nền MATE của tôi bị tối đen sau khi Fresco gặp sự cố?

Để giữ các biểu tượng màn hình nền của Caja nằm trên hình nền động, Fresco đặt
nền màn hình nền của MATE thành một màu khóa gần như đen (`#010101`) trong lúc
chạy và sao chép mọi thứ không phải màu đó — tức các biểu tượng — lên hình nền.
Nền riêng của bạn được lưu trước và được khôi phục khi Fresco dừng hoặc bị tắt.
Nếu Fresco gặp sự cố trước khi kịp làm vậy, màn hình nền sẽ vẫn tối;
**hãy khởi động lại Fresco và nó sẽ khôi phục nền của bạn**. Trên máy chủ X
không có các phần mở rộng Composite và Damage, Fresco hoạt động như trên
[Deepin](#biểu-tượng-màn-hình-nền-bị-ẩn-khi-hình-nền-chạy-trên-deepin):
biểu tượng bị ẩn, và nhấp vào màn hình nền sẽ đưa chúng trở lại.

### Hình nền bị đen trên Wayland (NVIDIA, COSMIC, Hyprland, Sway)

Hình nền đen — trong khi mọi thứ khác vẫn chạy và không có lỗi nào trong nhật
ký — hầu như luôn có nghĩa là trình kết xuất mpvpaper mà Fresco đóng kèm quá cũ
so với hệ thống của bạn. Các phiên bản trước 1.6 khởi tạo EGL, báo thành công
rồi không bao giờ xuất một khung hình nào trên trình điều khiển độc quyền của
NVIDIA.

Hãy chạy `fresco doctor` trước. Nó in ra trình kết xuất đã chọn, tệp nhị phân đó
đến từ đâu (đóng kèm hay do bạn tự cài) và đại khái là phiên bản nào, đồng thời
cảnh báo khi phiên bản đó cũ hơn bản sửa lỗi:

```
  ✓ mpvpaper available (/usr/lib/fresco/mpvpaper-libmpv2)
      source: bundled · version: 1.4–1.6
  ⚠ mpvpaper may be too old …
```

Fresco vốn đã ưu tiên `mpvpaper` mới hơn trong `PATH` của bạn so với bản đóng
kèm cũ hơn, nên thường chỉ cần cài gói `mpvpaper` của bản phân phối (hoặc biên
dịch [bản gốc](https://github.com/GhostNaN/mpvpaper) bằng
`scripts/build-mpvpaper.sh`) là đủ. Để trỏ Fresco tới một tệp nhị phân cụ thể,
hãy đặt **`FRESCO_MPVPAPER`** thành đường dẫn đầy đủ của nó — giá trị này ghi
đè mọi lựa chọn khác:

```bash
mkdir -p ~/.config/environment.d
echo 'FRESCO_MPVPAPER=/home/YOU/.local/bin/mpvpaper' > ~/.config/environment.d/fresco.conf
```

Đăng xuất rồi đăng nhập lại (systemd đọc `environment.d` khi phiên bắt đầu),
sau đó dùng `fresco doctor` để xác nhận dòng `source:` giờ ghi là
`FRESCO_MPVPAPER override`.

Tương tự, **`FRESCO_HWDEC`** ghi đè bộ giải mã phần cứng của mpv (ví dụ `nvdec`,
`nvdec-copy`, `vaapi`, `no`) — một công cụ chẩn đoán cho tình trạng dùng CPU cao.
Theo mặc định, Fresco chọn `nvdec,vaapi,auto-safe` khi có GPU NVIDIA, nếu không
thì `auto-safe` (video bị xoay dùng copy-back: `nvdec-copy,auto-copy` /
`auto-copy`).

## Quyền riêng tư và điều khoản sử dụng

**Không có gì được gửi đi cho đến khi bạn trả lời.** Một hộp thoại đồng ý chỉ hỏi một lần ở lần chạy đầu tiên, và lựa chọn có thể thay đổi bất cứ lúc nào trong Cài đặt.

Dù chọn gì, mỗi ngày một lần Fresco ghi nhận rằng có một bản cài đặt đã hoạt động, ở quốc gia nào, trên phiên bản nào. Đó là lượt đếm. Điều hộp thoại hỏi là **thông tin chi tiết**:

| Bạn chọn | Fresco gửi những gì |
| --- | --- |
| **Chấp nhận tất cả** | Lượt đếm, cộng thêm bản phân phối, desktop, loại phiên, backend video, số màn hình, các tính năng bạn dùng, loại lỗi, thành phố và khu vực, cùng thời điểm chính xác của mỗi lần check-in. |
| **Từ chối mục tùy chọn** | Chỉ lượt đếm: mã cài đặt ngẫu nhiên (không bao giờ được suy ra từ phần cứng hay tên của bạn), quốc gia, phiên bản ứng dụng, kiểu đóng gói. Lần check-in của bạn được lưu dưới dạng **ngày, không phải giờ**. |

**Thành phố và thời điểm sử dụng chính xác là tùy chọn** — chỉ được gửi nếu bạn chấp nhận tất cả, không bao giờ được gửi nếu bạn từ chối. Tọa độ không bao giờ bị thu thập ở bất kỳ mức nào: endpoint định vị địa lý bỏ vĩ độ và kinh độ chứ không trả về chúng.

Các con số này tồn tại để quyết định bản phân phối và desktop nào được kiểm thử trước mỗi bản phát hành, và nơi nào cần máy chủ mirror cho việc tải về. Chúng không bao giờ bị bán, chia sẻ hay dùng cho quảng cáo, và không có nhà cung cấp phân tích nào tham gia.

**Dù chọn gì cũng không bao giờ thu thập:** dữ liệu cá nhân, tên tệp, hình nền của bạn, địa chỉ IP của bạn, âm thanh, thao tác gõ phím hay nội dung clipboard. Quốc gia được Cloudflare xác định từ IP của bạn ở rìa mạng, nên chỉ có mã hai chữ cái đến được dự án này. (Văn bản bạn chủ động viết và bấm gửi — một nhận xét phản hồi, hay tin nhắn cho người phát triển ở bên dưới — là tin nhắn bạn chọn gửi đi, không phải thu thập dữ liệu.)

Để không gửi gì cả, hãy đặt `telemetry = false` và `telemetry_prompted = false` trong `~/.config/fresco/config.toml`.

### Nhắn tin ẩn danh cho người phát triển

**Menu → Nhắn tin cho người phát triển** mở một cuộc trò chuyện hai chiều riêng tư với người làm ra Fresco. Không cần tài khoản, không cần địa chỉ email, không cần đăng nhập GitHub. Nó ẩn danh theo cả hai chiều: bạn không bao giờ biết họ là ai ngoài "người phát triển", và họ không bao giờ biết bạn là ai — chỉ có tin nhắn của bạn và, nếu bạn để ô được đánh dấu, bản tóm tắt cấu hình hiển thị trong hộp thoại.

Cuộc trò chuyện được gắn với một phiếu ngẫu nhiên tạo riêng khỏi telemetry và lưu trong một tệp khác, nên không bao giờ có thể ghép một cuộc trò chuyện với hồ sơ sử dụng. Nó hoạt động hoàn toàn giống nhau dù bạn đã chấp nhận tất cả hay từ chối mục tùy chọn. Không có gì tồn tại cho đến khi bạn gửi tin nhắn đầu tiên.

📄 **[Điều khoản sử dụng và chính sách quyền riêng tư đầy đủ →](TERMS.md)** — từng trường dữ liệu, trong một bảng, không bỏ sót gì. Mọi dòng có gửi dữ liệu đi đều nằm trong [`src/telemetry.rs`](src/telemetry.rs), nên bạn có thể tự kiểm tra thay vì chỉ tin.

## Thông tin dự án

| | |
|---|---|
| **Tên** | Fresco (ID ứng dụng `io.github.dibbayajyotiroy.Fresco`) |
| **Đây là gì** | Ứng dụng hình nền động và hình nền video cho desktop Linux |
| **Giấy phép** | [GPL-3.0-or-later](LICENSE) — miễn phí và mã nguồn mở, không quảng cáo, không cần tài khoản |
| **Ngôn ngữ và công nghệ** | Rust, GTK4 / libadwaita, libmpv; trình kết xuất mpvpaper đóng kèm trên Wayland |
| **Bản phát hành đầu tiên** | v0.0.1, tháng 6 năm 2026 |
| **Phiên bản mới nhất** | 1.1.51 (2026-10-23) — xem [Releases](https://github.com/DibbayajyotiRoy/fresco/releases/latest) để biết bản mới nhất |
| **Gói** | `.deb` (amd64), tập lệnh cài đặt một dòng, Cửa hàng ứng dụng Deepin; biên dịch từ mã nguồn ở nơi khác |
| **Ngôn ngữ giao diện** | Tiếng Anh cùng 12 bản dịch: Trung giản thể, Nga, Tây Ban Nha, Pháp, Đức, Bồ Đào Nha (Brazil), Thổ Nhĩ Kỳ, Việt, Indonesia, Hindi, Bengal và Romania |
| **Người dùng** | 3.000+ người ở 128 quốc gia |
| **Người phát triển** | Dibbayajyoti Roy |
| **Mã nguồn** | <https://github.com/DibbayajyotiRoy/fresco> |
| **Trang web** | <https://fresco.dibbayajyoti.com> |

## Đóng góp và phản hồi

Rất hoan nghênh báo cáo lỗi, đề xuất tính năng và PR — hãy mở một [issue](https://github.com/DibbayajyotiRoy/fresco/issues), hoặc dùng hộp thoại phản hồi ngay trong ứng dụng. Xem [CONTRIBUTING.md](CONTRIBUTING.md).

## Giấy phép

[GPL-3.0-or-later](LICENSE) — miễn phí và mã nguồn mở.

---

<sub>Fresco là ứng dụng hình nền động và hình nền video miễn phí, mã nguồn mở cho Linux (X11 và Wayland), với các widget lời bài hát, đồng hồ, hiệu ứng hình ảnh âm thanh và ảnh bìa album được vẽ thẳng lên hình nền. Đây là lựa chọn thay thế Wallpaper Engine chạy native trên Linux, đồng thời là người bạn đồng hành về nhạc và thời gian cho các công cụ như Conky. Được 3.000+ người ở 128 quốc gia sử dụng. Cập nhật lần cuối: 2026-10-23.</sub>
