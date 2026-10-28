<div align="center">
<img src="./data/icons/hicolor/256x256/apps/io.github.dibbayajyotiroy.Fresco.png" width="112" alt="Fresco 应用图标——免费开源的 Linux 动态壁纸应用" />

# Fresco — 免费开源的 Linux 动态壁纸与视频壁纸软件

[English](README.md) | [العربية](README.ar.md) | [বাংলা](README.bn.md) | [Deutsch](README.de.md) | [Español](README.es.md) | [Français](README.fr.md) | [हिन्दी](README.hi.md) | [Bahasa Indonesia](README.id.md) | [Português (Brasil)](README.pt-BR.md) | [Română](README.ro.md) | [Русский](README.ru.md) | [Türkçe](README.tr.md) | [Tiếng Việt](README.vi.md) | **简体中文**

**Fresco 是一款免费、开源的 Linux 动态壁纸应用，可将任意视频、GIF 或图片设为动态桌面壁纸，同时支持 X11 与 Wayland。** 它是原生 Linux 平台上 Wallpaper Engine 的替代品，提供图形界面、GPU 硬件加速播放和桌面挂件，适合用来做 Linux 桌面美化。

[![Fresco 最新发布版本](https://img.shields.io/github/v/release/DibbayajyotiRoy/fresco?style=flat-square&label=release)](https://github.com/DibbayajyotiRoy/fresco/releases/latest)
[![GPL-3.0-or-later 许可证](https://img.shields.io/github/license/DibbayajyotiRoy/fresco?style=flat-square)](LICENSE)
[![发布流程状态](https://img.shields.io/github/actions/workflow/status/DibbayajyotiRoy/fresco/publish.yml?style=flat-square&label=publish)](https://github.com/DibbayajyotiRoy/fresco/actions/workflows/publish.yml)
[![GitHub 星标数](https://img.shields.io/github/stars/DibbayajyotiRoy/fresco?style=flat-square)](https://github.com/DibbayajyotiRoy/fresco/stargazers)

[官网](https://fresco.dibbayajyoti.com) · [安装](#安装) · [支持的环境](#支持的环境) · [对比](#fresco-与-wallpaper-engine-等其他-linux-动态壁纸方案的对比) · [常见问题](#常见问题) · [桌面挂件](#桌面挂件) · [更新日志](CHANGELOG.md) · [问题反馈](https://github.com/DibbayajyotiRoy/fresco/issues)

<img src="./data/screenshots/gallery_zh.png" alt="Fresco 中文界面的壁纸库窗口：按文件夹分组的视频壁纸网格，带搜索栏，卡片上标注分辨率、帧率和文件大小，底部有“添加文件夹”“从链接添加”“添加”和“选择”按钮" width="800" />

</div>

## Fresco 是什么？

Fresco 通过 GTK4 / libadwaita 图形界面，把视频、GIF、图片、幻灯片或播放列表设为窗口背后的动态桌面壁纸，全程无需终端。播放经 mpv 完成，解码在 GPU 上进行，CPU 占用保持在接近空闲的水平——实际功耗见[性能](#性能与续航)。每次登录后壁纸都会自动恢复。

- **支持的媒体：** 视频（mp4、webm、mkv、avi、mov）、动态 GIF、图片、图片幻灯片和视频播放列表。
- **支持的桌面：** X11 会话（Xorg 下的 GNOME、Cinnamon、Xfce、MATE、Deepin 25）与 Wayland（COSMIC、Hyprland、Sway、KDE Plasma 6、Cinnamon 6.8 及以上）。GNOME on Wayland 只能显示静帧——见[支持的环境](#支持的环境)。
- **硬件解码：** 通过 mpv 使用 VA-API（Intel、AMD）和 NVDEC（NVIDIA）。
- **壁纸库：** 可直接链接你已有的壁纸文件夹而无需导入，文件增删后自动保持同步。壁纸库只绘制屏幕上可见的部分，数千张壁纸也依然流畅轻量。
- **桌面挂件：** 逐句同步的歌词、时钟（含世界时钟）、音频可视化与专辑封面，均绘制在壁纸内部，默认全部关闭。
- **操作方式：** 应用界面、Ctrl+K 命令面板，以及可绑定快捷键的 `fresco next` / `fresco prev` / `fresco random`。
- **安装：** 面向 Ubuntu、Debian、Linux Mint、Pop!_OS、elementary OS、Kali 与 Deepin 的 `.deb` 包（一行脚本、手动下载或 Deepin 应用商店）；Fedora、Arch 等发行版从源码构建——见[安装](#安装)。
- **许可证：** 免费开源，采用 [GPL-3.0-or-later](LICENSE)，无广告，无需注册账号。
- **用户规模：** 来自 128 个国家/地区的 3,000+ 名用户。最新版本：1.1.53。

## 安装

Fresco 为 Debian 系发行版提供 `.deb` 包，其他发行版从源码构建。

### Ubuntu、Debian、Linux Mint、Pop!_OS、elementary OS 与 Kali Linux

一行命令安装——从 GitHub Releases 下载最新的 `.deb` 并用 apt 安装：

```bash
curl -fsSL https://github.com/DibbayajyotiRoy/fresco/releases/latest/download/install.sh | FRESCO_SOURCE=github bash
```

或从 [Releases](https://github.com/DibbayajyotiRoy/fresco/releases/latest) 下载 `.deb` 并执行：

```bash
sudo apt install ./fresco_*.deb
```

### Deepin 25

打开**应用商店**，搜索 **Fresco**，点击**安装**。Fresco 已上架 deepin 社区应用商店，无需手动下载，更新随商店推送。上面的一行命令安装方式在 Deepin 上同样可用。

### Fedora、Arch Linux 及其他发行版

这些发行版暂无预编译包，需要从源码构建。你需要 Rust（通过 [rustup](https://rustup.rs) 安装），以及 GTK4、libadwaita 和 libmpv 的开发头文件：

```bash
git clone https://github.com/DibbayajyotiRoy/fresco.git && cd fresco
cargo build --release --all-features
```

在 Wayland 合成器上，Fresco 还需要 `mpvpaper` 渲染器——`.deb` 包已内置，源码构建则没有：

```bash
# Arch Linux、CachyOS、EndeavourOS（AUR）
yay -S mpvpaper

# Fedora 及其他发行版：针对系统的 libmpv 自行构建
scripts/build-mpvpaper.sh
install -Dm755 target/release/mpvpaper ~/.local/bin/mpvpaper   # 必须在 PATH 中
```

Fresco 目前不在 AUR 或 Flathub 上——AUR 中名为 `fresco` 的包是一个无关的物理计算程序。详细说明与故障排查见 [docs/INSTALL.md](docs/INSTALL.md)。

安装完成后，从应用启动器打开 **Fresco**。如果运行不正常，请执行 `fresco doctor`。

## 如何在 Linux 上把视频设为壁纸

安装 Fresco，点击**添加**，选好视频，再点击**设为壁纸**。视频会在窗口背后播放，并在每次登录时自动恢复。

1. 安装 Fresco（见上），从应用启动器打开；
2. 点击**添加**，选择一个视频、GIF 或图片——或用**从链接添加**粘贴链接；
3. 可选：在编辑器中裁剪或旋转；
4. 点击**设为壁纸**，然后关闭应用。

窗口关闭后壁纸继续播放。

## 功能特性

- **任意媒体**——循环视频（mp4/webm/mkv）、动态 GIF、静态图片、图片幻灯片、多视频播放列表
- **从链接添加**——粘贴 Pinterest 图钉链接或任意视频/图片的直接 URL；Fresco 会自动下载并打开裁剪编辑器
- **链接文件夹**——直接使用你已有的壁纸文件夹（图片、GIF 和视频），无需导入；增删文件后自动保持同步，子文件夹可选，收藏与编辑按文件分别保留
- **大型壁纸库**——壁纸库只绘制滚动位置附近的壁纸，因此数千张壁纸约两秒即可打开，内存占用只有几百兆（在 4,000 张壁纸的测试库上：约 2 秒、325 MB，此前为 9 到 13 秒、1.6 GB）
- **硬件解码**——通过 GPU 进行视频解码（VA-API / NVDEC），将解码工作转移到视频引擎上，让 CPU 占用率接近空闲水平。实测显示这种方式成本明显更低（但并非零开销——详见[性能](#性能与续航)部分）
- **省电模式**——为笔记本提供更低的 GPU 缩放开销；实测可将 GPU 功耗降低约一半（见[性能](#性能与续航)）
- **多显示器**——每块屏幕可设不同壁纸，同一视频可跨屏同步播放
- **昼夜壁纸**——按定时、任意时间段或日出/日落切换壁纸
- **桌面挂件**——同步歌词、主题时钟、世界时钟、音频可视化、旋转的专辑封面圆盘，直接绘制在壁纸内部，不会遮挡你的窗口（世界时钟另有可选的“悬浮在窗口之上”模式）；默认全部关闭（见[桌面挂件](#桌面挂件)）
- **锁屏**——在你的真实锁屏上显示壁纸和挂件（时钟、问候语、正在播放、电池等），替代系统默认锁屏——COSMIC 1.9+ 与 KDE Plasma 6 支持动态视频与挂件，Sway/Hyprland/niri 与 X11 通过 `fresco lock` 显示壁纸与挂件，其他环境显示静帧。默认关闭，且 Fresco 永远看不到你的密码——见 [docs/LOCKSCREEN.md](docs/LOCKSCREEN.md)
- **键盘控制**——`fresco next`、`fresco prev`、`fresco random` 可绑定到合成器快捷键（Fresco 窗口已打开时，它会采用新壁纸而不会改回去），应用内还有 Ctrl+K 命令面板
- **批量管理**——一次选中多张壁纸，一键移除
- **内置目录**——在应用内浏览精选的、授权合规的壁纸
- **全屏自动暂停**——在每台显示器上，支持X11、wlroots合成器、COSMIC和KDE Plasma（通过其壁纸插件）；也可选择适用于最大化窗口，并支持电池供电时暂停。
- **浏览器新标签页扩展**——在每个新标签页镜像你的壁纸（Chrome/Brave/Edge/Firefox；从 [`./extension`](extension) 以未打包方式加载）
- **Deepin DDE 支持**——在 Deepin 25 上自动适配 DDE 桌面，需要时点击桌面即可让图标重新显示十秒（见[故障排查](#deepin-上播放动态壁纸时桌面图标被隐藏了)）
- **裁剪与旋转编辑器**——每张壁纸独立的音频/音量、幻灯片切换效果、可搜索的壁纸库

## 支持的环境

| 环境 | 动态壁纸 | 说明 |
|---|---|---|
| X11（GNOME、Cinnamon、XFCE 等） | ✅ | 内嵌渲染器。GNOME 仅在发行版仍提供 Xorg 会话时可用（见下方 GNOME 说明） |
| Kali Linux（Xfce，X11） | ✅ | 在 Kali 默认的 Xfce-on-X11 会话上运行，与其他 X11 桌面使用同一内嵌渲染器 |
| Xfce（X11） | ✅ | 桌面图标在壁纸之上保持可见，且可以点击、选中和拖动 |
| MATE（X11） | ✅ | 桌面图标在壁纸之上保持可见、可点击 |
| Deepin 25（DDE，X11） | ✅ | 自动 DDE 适配——社区已在 Deepin 25 Community build1 上验证 |
| COSMIC（Wayland） | ✅ | layer-shell |
| Hyprland | ✅ | layer-shell |
| Sway | ✅ | layer-shell |
| Cinnamon 6.8 及以上（Wayland） | ✅ | Cinnamon 的合成器自 6.8 起支持 layer-shell。Cinnamon 6.6 及更早版本没有 layer-shell，在 Wayland 上保持静帧 |
| KDE Plasma 6（X11 和 Wayland） | ✅ | 通过 plasmashell 自带的壁纸插件进行设置，因此桌面图标保持可见。视频播放需要 `qml6-module-qtmultimedia`。播放由 Qt 负责，而非 mpv：静音、无硬件解码调优、无裁剪或转场；播放列表仅播放其第一个文件，幻灯片仅显示其第一帧。**填充方式**选项（填充、适应、拉伸）会生效。 |
| GNOME on Wayland | ⚠️ | 仅静帧——Mutter 不提供动态壁纸绘制面，无视频、无挂件。动态视频需要一个 Fresco GNOME 扩展（规划中） |

以上每个环境都在每次发布时于 CI 中无头运行验证。

**GNOME 说明。** GNOME on Wayland 是唯一一个 Fresco 只能显示静帧的主流桌面。过去绕开的办法是登录 Xorg 会话，而这条路正在消失：GNOME 49 禁用了其 X11 会话，GNOME 50 移除了它，因此 Ubuntu 25.10 及更新版本（含 26.04 LTS）与 Fedora 43 及更新版本不再提供可切换的 Xorg 会话。Ubuntu 22.04 和 24.04 仍提供 "Ubuntu on Xorg"。在没有 Xorg 会话的系统上，原版 GNOME 桌面的动态视频只能等待规划中的 Fresco GNOME 扩展；Fresco 会在应用内如实说明，而不是假装视频在播放。KDE Plasma、COSMIC、Hyprland、Sway 以及所有 X11 桌面不受影响。

> “界面简洁易用——是为数不多对 Deepin 25 适配得当的动态壁纸应用，可通过 .deb 安装，硬件加速播放流畅。”
>
> — 柒玖（deepin 论坛）/ 柒仈玖（GitHub），测试环境：Deepin 25 Community build1，X11 会话，Intel Alder Lake-N [Intel Graphics]

Deepin 25 默认会话为 X11，Fresco 在该系统上即以此会话验证。Deepin 自家的 Wayland 合成器 [Treeland](https://github.com/linuxdeepin/treeland) 仍在开发中，Fresco 暂不对 Deepin on Wayland 作任何承诺。

## Fresco 与 Wallpaper Engine 等其他 Linux 动态壁纸方案的对比

| 工具 | 平台 | Wayland | X11 | 操作界面 | 支持的内容 | 许可证 |
|---|---|---|---|---|---|---|
| **Fresco** | Linux | ✅ layer-shell 合成器与 KDE Plasma 6；GNOME 上为静帧 | ✅ | GTK4 应用，另有 `fresco` 命令行 | 视频、GIF、图片、幻灯片、播放列表 | GPL-3.0-or-later |
| **Wallpaper Engine** | Windows（Steam）；无官方 Linux 版本 | 不适用 | 不适用 | 图形界面 | 视频、场景与网页壁纸 | 付费、闭源 |
| **[linux-wallpaperengine](https://github.com/Almamu/linux-wallpaperengine)** | Linux；需要 Steam 版 Wallpaper Engine 的资源文件 | ✅ 支持 layer-shell 的合成器 | ⚠️ 当桌面环境（GNOME、KDE）绘制背景时不可用 | 命令行；其 README 列出了社区开发的独立图形界面 | Wallpaper Engine 壁纸 | GPL-3.0 |
| **[Hidamari](https://github.com/jeffshee/hidamari)** | Linux，侧重 GNOME | ⚠️ 仅 GNOME Wayland | ✅ | 图形界面 | 视频 | GPL-3.0 |
| **[Komorebi](https://github.com/cheesecakeufo/komorebi)** | Linux | — | — | 图形界面 | 图片、视频与网页壁纸 | GPL-3.0 |
| **[mpvpaper](https://github.com/GhostNaN/mpvpaper)** | Linux，基于 wlroots 的合成器 | ✅ layer-shell | ❌ | 命令行 | 视频（通过 mpv） | GPL-3.0 |
| **xwinwrap** | Linux，仅 X11 | ❌ | ✅ | 命令行 | 你挂接的任意播放器（mpv、gifview） | — |
| **swww**（现名 [awww](https://codeberg.org/LGFae/awww)） | Linux，layer-shell 合成器；不支持 GNOME | ✅ | ❌ | 命令行守护进程 | 动态 GIF 与图片，不含视频 | GPL-3.0 |

Fresco 将 `mpvpaper` 作为其 Wayland 渲染器打包，因此其是构建于该项目之上而非与其竞争。`—` 表示该项目自己的文档未明确说明。以上信息核对自各项目于 2026-10-16 的公开仓库：Komorebi 最近一次仓库推送在 2025 年 7 月，swww 的 GitHub 仓库已归档，因为项目已迁至 Codeberg 并更名为 awww。

## 性能与续航

柒玖（deepin 论坛）/ 柒仈玖（GitHub）在一台 Intel N150 上（Deepin 25，VA-API，每档各测两次）用 `turbostat` 实测了播放视频壁纸时的封装功耗：

| 视频 | 省电档位 | GPU 功耗 | 封装总功耗 |
|---|---|---|---|
| 1080p 60fps | 完整画质 | 1.37 W | 6.00 W |
| 1080p 60fps | **较低**（默认） | **0.63 W**（−54%） | **4.03 W**（−33%） |
| 4K 60fps | 完整画质 | 2.77 W | 7.94 W |
| 4K 60fps | **较低**（默认） | **1.60 W**（−42%） | **5.95 W**（−25%） |
| 4K 60fps | 最低画质 | 0.99 W（−65%） | 4.97 W（−37%） |

省电档位降低的是每帧的 GPU 缩放开销。不掉帧、硬件解码不受影响，因此播放依旧流畅——代价是画面锐度，而非流畅度。默认档为“较低”；“最低画质”在使用 4K 片源值得选择。

**如何解读这些数字。** 硬件解码并不意味着动态壁纸零开销——它把工作从 CPU 移到了能效更高的视频引擎上，而不是让工作消失。单看“CPU 占用低”是个弱指标，因为与之对比的替代方案是软件解码，后者差得多；诚实的度量是整机功耗，这正是表格报告**封装总功耗**而不只是 GPU 功耗的原因。

对上表数据的两点保留，以免被过度解读：

- 它们是**壁纸播放时的封装总功耗**，不是壁纸本身的边际开销。同一台机器上没有录空载基线，因此这些数字与空闲桌面之间的差值在此无法确立。百分比只用于省电*档位*之间的互比，这才是它们有效的用途。
- 它们来自一台机器（Intel N150，Alder Lake-N，VA-API，Deepin 25，每档两次）。独立显卡、NVDEC 及其他驱动会有所不同。

动态壁纸的功耗必然高于静态壁纸。省电档位、全屏自动暂停和电池供电时暂停的存在是为了约束这份开销，而不是假装它为零。

## 常见问题

### Linux 上有 Wallpaper Engine 的替代品吗？

有。Fresco 是一款免费开源的 Linux 应用，通过图形界面播放你自己的视频、GIF 和图片壁纸，支持硬件解码，无需兼容层。它不能打开 Wallpaper Engine 自己的壁纸文件；如果你已购买 Wallpaper Engine 并想在 Linux 上运行那些壁纸，可以使用独立项目 [linux-wallpaperengine](https://github.com/Almamu/linux-wallpaperengine)——它通过命令行运行，并需要 Steam 版 Wallpaper Engine 的资源文件。见[对比表](#fresco-与-wallpaper-engine-等其他-linux-动态壁纸方案的对比)。

### Wallpaper Engine 在 Linux 上能用吗？

不能原生运行。Wallpaper Engine 是 Windows 应用，没有官方 Linux 版本。Fresco 是原生 Linux 替代品，以 `.deb` 安装，无需兼容层。

### 在 Ubuntu 上怎么把视频设为壁纸？

安装 Fresco，打开它，点**添加**，选好视频，点**设为壁纸**。视频即作为桌面背景播放，并在登录时恢复播放。Ubuntu 默认的 GNOME-on-Wayland 会话只能显示静帧。在 Ubuntu 22.04 和 24.04 上可登录 **Ubuntu on Xorg** 会话获得完整的动态播放；Ubuntu 25.10 及更新版本（含 26.04 LTS）不提供 Xorg 会话，在这些系统上，原版 GNOME 桌面的动态视频需要规划中的 Fresco GNOME 扩展。使用 KDE Plasma、Xfce 或 MATE 的 Ubuntu 衍生版不受影响。

### 怎么在 Ubuntu、Linux Mint、Pop!_OS、Fedora 或 Arch Linux 上用动态壁纸？

在 Ubuntu、Linux Mint 和 Pop!_OS 上安装 Fresco 的 `.deb` 包，在 Fedora 和 Arch Linux 上则从源码构建。各自的命令见[安装](#安装)。

- **Ubuntu：** `.deb`。在默认的 GNOME 桌面上，动态播放需要 Xorg 会话（Ubuntu 22.04 和 24.04）；KDE Plasma、Xfce 和 MATE 衍生版不受影响——见上面的 Ubuntu 回答。
- **Linux Mint：** `.deb`。支持 X11 下的 Cinnamon，以及 6.8 及以上版本的 Wayland 下的 Cinnamon。
- **Pop!_OS：** `.deb`。支持 COSMIC 桌面（Wayland layer-shell）。
- **Fedora：** 从源码构建。Fedora 43 及更新版本的 GNOME 仅有 Wayland，只能显示静帧；KDE Plasma 与 X11 桌面受支持。
- **Arch Linux：** 从源码构建，并从 AUR 安装 `mpvpaper`（`yay -S mpvpaper`）。

只有 `.deb` 这条路径被 Fresco 的 CI 覆盖。

### 支持哪些 Linux 发行版？

Fresco 为 Debian 系与 Ubuntu 系发行版提供 `.deb` 包：Ubuntu、Pop!_OS、Linux Mint、Debian、elementary OS、Deepin 25 与 Kali Linux。其他发行版可从源码构建——见[安装](#安装)和 [docs/INSTALL.md](docs/INSTALL.md)。

### 动态壁纸很消耗 CPU、GPU 或电池吗？

CPU 方面，不会。电池方面，会消耗一些——动态壁纸永远不可能零开销。

Fresco 在 GPU 上解码视频（VA-API / NVDEC），CPU 占用保持接近空闲。这只是把开销转移到视频引擎，而不是消除它。实测 Intel N150 在默认省电档下，1080p 壁纸播放时 GPU 功耗 0.63 W、封装总功耗 4.03 W（如何解读见[性能](#性能与续航)，包括它*不能*说明什么）。全屏自动暂停与电池供电时暂停，就是为了在最要紧的时候把这份开销从你的电池上挡开。

### Fresco 支持 Wayland 吗？

支持，在实现了 layer-shell 协议的合成器上——COSMIC、Hyprland 与 Sway——以及 KDE Plasma 6（X11 和 Wayland，通过 plasmashell 自带的壁纸插件设置）。GNOME on Wayland 是例外：Mutter 不提供壁纸绘制面，Fresco 在那里显示静帧并在应用内说明。X11 会话则完全支持。

### Fresco 支持 GNOME 吗？

X11 上完全支持，包括完整的动态壁纸——视频、挂件，一切照常。在 **GNOME on Wayland** 上，Mutter 不为任何应用提供可绘制的壁纸面，Fresco 只能显示静帧，挂件也不可用。如果你的发行版仍提供 Xorg 会话（Ubuntu 22.04 和 24.04 提供），注销后选择 "GNOME on Xorg" 或 "Ubuntu on Xorg" 即可获得完整的动态播放。GNOME 49 禁用了该会话、GNOME 50 移除了它，因此 Ubuntu 25.10 及更新版本（含 26.04 LTS）与 Fedora 43 及更新版本没有 Xorg 会话：在这些系统上，GNOME 的动态视频需要规划中的 Fresco GNOME 扩展。`fresco doctor` 会告诉你属于哪种情况。

### Fresco 支持 KDE Plasma、COSMIC、Hyprland 和 Sway 吗？

支持，四者皆可。COSMIC、Hyprland 和 Sway 通过 layer-shell 播放动态视频。KDE Plasma 6（X11 和 Wayland）使用 Fresco 在 plasmashell 内的专用壁纸插件，桌面图标保持可见，但播放由 Qt 而非 mpv 负责：静音、无裁剪与转场，视频播放还需要 `qml6-module-qtmultimedia`。**填充方式**选项（填充、适应或拉伸）在这里同样适用。

### Fresco 支持 Linux Mint、Xfce、Cinnamon 和 MATE 吗？

支持。X11 下的 Cinnamon、Xfce 和 MATE 使用内嵌渲染器，在 Xfce 和 MATE 上桌面图标会保持显示在壁纸之上；在 Xfce 上你还可以照常点击、选中和拖动它们。Wayland 下的 Cinnamon 自 6.8 起支持，该版本加入了 layer-shell 支持。

### 怎么用快捷键切换壁纸？

在合成器或桌面环境的设置里，把 `fresco next`、`fresco prev` 或 `fresco random` 绑定到某个按键——它们无需打开应用即可切换壁纸（`random` 不会选中正在播放的那张）。它们按应用中显示的顺序在你的壁纸库中依次切换，或只在你当前打开的文件夹内切换，并在 Fresco 服务未运行时将其启动。如果此时 Fresco 窗口是打开的，它会采用新壁纸，而不会把旧壁纸改回去。应用内按 Ctrl+K 可打开命令面板。例如在 Sway 中：

```
bindsym $mod+Right  exec fresco next
bindsym $mod+Left   exec fresco prev
bindsym $mod+Return exec fresco random
```

### 每个显示器可以用不同的壁纸吗？

可以。Fresco 支持按显示器设置壁纸，且同一视频跨多屏使用时播放保持同步。

### 支持 GIF 和图片幻灯片吗？

支持——除视频文件外，还有动态 GIF、静态图片、带切换效果（淡入黑场再淡出、缓慢平移缩放（Ken Burns）、推近和虚化）的图片幻灯片，以及多视频播放列表。

### Fresco 能直接使用我现有的壁纸文件夹而不用导入吗？

能。在菜单中选择**链接文件夹…**（或先点**添加文件夹**，再选**链接此文件夹**），指向你平时存放壁纸的文件夹。其中的图片、GIF 和视频会留在原处，在你的壁纸库里以单独的分区显示，无需导入；你在该文件夹里增删文件，壁纸库会随之保持同步。**包含子文件夹**为可选项（最多向下四层），收藏与编辑会按文件分别记住。如果文件夹消失（比如拔掉了移动硬盘），它的分区会标上“（不可用）”并保留，待文件夹恢复后自动回来。**取消链接文件夹**（在分区菜单中）会移除该分区，但不会删除任何文件。**添加**仍然像以前一样导入单个文件。

### Fresco 能应付数千张壁纸吗？

能。壁纸库只为你滚动位置附近的壁纸构建卡片，而不是一次性构建全部，所以庞大的收藏不会拖垮应用。在一个 4,000 张壁纸（图片和视频各一半）的测试库上，壁纸库约 2 秒就出现，此前要 9 到 13 秒；内存占用约 325 MB，此前为 1.6 GB。这些数据来自一台机器上、负载较高时的开发版本，请当作参考，而不是承诺。

### 能把 YouTube 视频设为壁纸吗？

不能直接粘贴它的链接。**从链接添加**只接受 Pinterest 图钉链接，以及指向视频或图片文件的直接链接（mp4、webm、mkv、mov、gif、jpg、png、webp）；其他链接会被拒绝并给出提示。请用**添加**导入你已有的视频文件，或粘贴文件的直接链接。

### Fresco 是免费开源的吗？

是。Fresco 以 GPL-3.0-or-later 许可证免费开源。无广告、无账号、无付费档。

### Fresco 会收集我的数据吗？

在你回答一次性的同意对话框之前，什么都不会发送。你可以选择仅发送每日一次的匿名计数（随机安装 id、国家、应用版本），或在此之外再加上可选的详细信息；在 `~/.config/fresco/config.toml` 里设 `telemetry = false` 则完全不发送任何内容。每个字段见[隐私与使用条款](#隐私与使用条款)。

### Fresco 和 mpvpaper 有什么区别？

`mpvpaper` 是一个命令行 mpv 封装，把单个视频播为 Wayland layer-shell 背景——没有图形界面、没有壁纸库、不支持 X11。Fresco 是围绕它构建的 GTK4 桌面应用：壁纸库、裁剪/旋转编辑器、排程、多屏同步、桌面挂件，并支持 X11，全程无需终端。Fresco 将 `mpvpaper` 作为 Wayland 渲染器打包而非取代它——见[对比表](#fresco-与-wallpaper-engine-等其他-linux-动态壁纸方案的对比)。

### 能在 Kali Linux 上用 Fresco 吗？

能。Kali Linux 基于 Debian，Fresco 的安装方式与 Debian/Ubuntu 相同——`.deb` 包或一行安装脚本（见[安装](#安装)）。Kali 默认的 Xfce on X11 桌面通过 Fresco 的内嵌 X11 渲染器获得支持。

### 怎么一次移除多张壁纸？

点击底栏的**选择**（或右键一张壁纸选**选择…**），勾选要删的，点**移除**。**全选**遵循当前搜索，所以可以先搜索再整批清空。移除壁纸只是把它移出 Fresco 库——磁盘上的源文件保留。

### 怎么卸载 Fresco？

如果你是通过 `.deb` 或安装脚本安装的，执行 `sudo apt remove fresco`。你的壁纸库和配置位于 `~/.local/share/fresco/` 与 `~/.config/fresco/`，删除这两个目录即可彻底清除。如果只想恢复桌面自带的壁纸而不卸载，打开 Fresco 点击**停止壁纸**即可。

## 桌面挂件

Fresco 可以在壁纸上绘制五种可选挂件，包括桌面歌词和桌面时钟。全部**默认关闭**；从应用菜单（Ctrl+,）→ **高级…** 中开启。

| 挂件 | 效果 |
|---|---|
| **歌词** | 与音乐同步的当前行。四种预设（极简、卡拉OK、字幕、卡片）、九宫格位置、同步偏移滑块、可选的下一行暗显，以及可选的曲名与歌手 |
| **时钟** | 八种主题——数字、极简、数码管、层叠、文字、卡片、NOS 和手机锁屏风格。12/24 小时制，可选日期，可调颜色与不透明度。秒数默认关闭，因为它带来 60 倍的重绘开销 |
| **世界时钟** | 同时显示多个城市的时间，城市从完整的时区列表中选择，以 12 小时制纯文字显示（例如 `Tokyo  9:41 PM`），每个城市可加标签。每分钟重绘一次。1.1.50 新增 |
| **音频可视化** | 五种样式——柱状、镜像、波形、圆点、环形——支持取色器、双色渐变或彩虹 |
| **专辑封面** | 当前曲目封面在旋转的唱片上；播放暂停时停止旋转 |

除世界时钟可选的**悬浮在窗口之上**模式外，挂件通过 mpv 的 OSD 层绘制在壁纸内，而不是画在独立窗口里，因此永远不会盖在你的窗口上方、永远不会拦截点击，且在 X11 与所有 layer-shell 合成器上表现一致。实测在音乐播放、歌词、时钟、可视化与专辑封面挂件全开时开销为**单核 CPU 的 0.8%**——几乎全部来自音频采集，因为内容不变就不重绘。机器休眠并唤醒后，挂件也会自行恢复显示。

挂件默认出现在**每一块屏幕**上。想只留在一块屏上，可在 `config.toml` 的 `[widgets]` 块中加 `monitor = "DP-1"`（暂无图形界面开关）。挂件**在 GNOME on Wayland 上不可用**——那里没有可供 Fresco 绘制的动态壁纸面，壁纸在那里退回静帧也是同一原因。

### 能在 Linux 桌面壁纸上放一个时钟吗？

可以。开启**时钟**挂件，即可在壁纸上显示带主题的时钟；开启**世界时钟**则可同时显示多个城市的时间。世界时钟和其他挂件一样默认画在壁纸上；它可选的**悬浮在窗口之上**模式会把时钟抬到所有应用之上，点击可穿透——在带合成器的 X11 以及支持 layer-shell 的合成器（KDE Plasma、COSMIC、Sway、Hyprland）上可用，在无法悬浮的会话（例如 GNOME on Wayland）上则退回到壁纸上显示。

### 可以在 Linux 桌面上显示歌词吗？

可以。Fresco 把**逐时间同步的歌词**绘制到你的壁纸上，通过 MPRIS 跟随系统中正在播放的内容——浏览器、音乐应用、视频播放器皆可。在歌词挂件设置里选择预设、位置和同步偏移即可（见上表）。

### Linux 有类似 Conky 的桌面挂件吗？

有，而且 Fresco 提供的五个挂件不需要面板、不需要扩展、也不需要桌面环境配合：时钟、世界时钟、同步歌词、音频可视化与旋转的专辑封面圆盘。它们绘制在壁纸里而非窗口里，因此在不自带组件层的桌面上也能工作——包括 COSMIC、Hyprland 与 Sway。与 Conky 不同，Fresco **没有系统监控类挂件**（没有 CPU、内存、温度或网络读数），所以它与 Conky 是音乐与时间的互补关系，而非替代。唯一跑不了的地方是 GNOME on Wayland。

### 能在桌面背景上使音乐可视化吗？

能。Fresco 的音频可视化跟随系统正在播放的内容，共五种样式——柱状、镜像、波形、圆点或环形——支持取色器、双色渐变或彩虹。默认关闭，首次开启时会征求你的同意，因为它需要监听你的音频输出；该同意在加载配置时同样强制生效，所以手动改 `config.toml` 无法在背后悄悄打开它。可与专辑封面挂件搭配，得到旋转的当前曲目封面唱片。

### 哪些音乐播放器能配合歌词挂件？

任何发布标准 MPRIS 元数据的播放器都可以，但可靠性并不相同：

| 播放器 | 能用吗 | 说明 |
|---|---|---|
| **Firefox** | ✅ | 最可靠，本功能即针对它验证 |
| Chrome / Brave / Edge / Vivaldi / Opera | ⚠️ | 见下 |
| Spotify——**浏览器版** | ✅ | 正确上报播放位置 |
| Spotify——**Linux 原生客户端** | ⚠️ | 位置永远上报为 0，歌词无法保持同步 |
| 本地播放器（VLC、mpv、Rhythmbox 等） | ✅ | 也是最可能有本地 `.lrc` 文件的情形 |

Chromium 系浏览器在任意标签页开始播放媒体时就认领一个 MPRIS 名，**之后永不释放**——一个由来已久的 Chromium bug。播放结束后，标题被清空但总线上残留一个过期的“僵尸”会话，有时还带着封面。Fresco 忽略不发布曲名的播放器，从而跳过这些会话，但 Chromium 自身的上报仍然不够稳定，所以这一功能请用 Firefox。

Spotify 的 Linux 原生客户端自 2018 年起就返回 `Position: 0` 且从不发出 seek 事件，native 与 snap 包皆然。Fresco 通过行为特征检测这一情况——播放中三次间隔的零读数——转而从切歌时刻起让歌词时钟自由计时，这在你频繁跳转时会漂移。浏览器版 Spotify 没有这个问题。

### 歌词从哪里来？

优先用本地 `.lrc` 文件：与音频文件同目录的，或你在 Fresco 中指定的歌词目录里匹配的。这条路离线可用且匹配最佳，因为那是你自己选的文件。

没有本地文件时——如果你在线听歌，大多数时候如此——Fresco 到 [LRCLIB](https://lrclib.net)（免费、社区运营的同步歌词数据库）查询该曲目，并把结果缓存在 `~/.cache/fresco/lyrics`，同一首歌绝不重复抓取。该查询会把曲名、歌手与专辑发送给 LRCLIB；除非歌词挂件开启且曲目没有本地文件，否则什么都不会发送。Fresco 不托管、不持有、也不为歌词内容授权——LRCLIB 的条目由其用户贡献，LRCLIB 亦声明对这些内容不做授权，因此 Fresco 按需抓取并按用户缓存，而不自带歌词库。

`.lrc` 文件的时间轴是手工制作的，常常略有偏差。歌词设置里的同步偏移滑块就是为此准备的。

## 故障排查

请先运行 `fresco doctor`：它会报告你的会话类型、Fresco 选用了哪个渲染器及其来源，以及 GNOME 的 Xorg 变通办法是否适用。[docs/INSTALL.md](docs/INSTALL.md) 涵盖更多情况，包括 CPU 占用过高。提交 issue 时请附上 `fresco doctor` 的输出。

仍未解决？[报告 Bug](https://github.com/DibbayajyotiRoy/Fresco/issues/new?template=bug_report.yml) 或[提出功能建议](https://github.com/DibbayajyotiRoy/Fresco/issues/new?template=feature_request.yml)。

### 安装后 Fresco 没有出现在 Deepin 启动器里——为什么？

这是 dde-launchpad 的已知问题：它安装后的刷新抓不到 Fresco。运行 `killall dde-shell`（它会自动重启），或注销再登录，条目就会永久出现。Fresco 本身安装无误——Deepin 自家的应用管理器能列出它，其图标在所有已装主题中都能解析。如果在一台装过 Fresco 的机器上每次重装都遇到，可运行 `sh scripts/dde-launcher-diag.sh`——它是只读的，会输出一份值得附到 issue 里的日志。

### Deepin 上播放动态壁纸时桌面图标被隐藏了

Deepin 25 把壁纸和桌面图标画在同一个不透明窗口里，所以可见的动态壁纸只能叠在那个窗口之上——不存在介于两者之间的层。**点击桌面，图标会回来十秒**，足够打开你要的东西；之后壁纸恢复，再点一次再得十秒。要调整时长，可在 `~/.config/fresco/config.toml` 里设 `dde_icon_peek_secs`（`0` 表示壁纸永远置顶）。Fresco 关闭或暂停时，Deepin 自家壁纸照常显示。

### 崩溃后我的 MATE 桌面背景看起来发黑

为了让 Caja 的桌面图标保持在动态壁纸之上，Fresco 运行时会把 MATE 的桌面背景设为一个近黑色的关键色（`#010101`），并把所有不是该颜色的内容——即图标——拷贝到壁纸之上。你原来的背景会先被保存，在 Fresco 停止或被禁用时放回。如果 Fresco 在此之前崩溃，桌面就会保持黑色；**重新启动 Fresco 即可恢复你的背景**。在没有 Composite 与 Damage 扩展的 X 服务器上，Fresco 改为采用与 [Deepin](#deepin-上播放动态壁纸时桌面图标被隐藏了)相同的做法：隐藏图标，点击桌面找回。

### Wayland 上我的壁纸是黑的（NVIDIA、COSMIC、Hyprland、Sway）

壁纸全黑——其他一切正常、日志无错——几乎总意味着 Fresco 打包的 mpvpaper 渲染器对你的环境太旧。1.6 之前的版本在 NVIDIA 专有驱动上初始化 EGL、报告成功，然后永远不呈现任何帧。

先运行 `fresco doctor`。它会打印它选中的渲染器、该二进制的来源（打包内置还是你自己安装的）以及大致版本，并在版本早于修复时给出警告：

```
  ✓ mpvpaper available (/usr/lib/fresco/mpvpaper-libmpv2)
      source: bundled · version: 1.4–1.6
  ⚠ mpvpaper may be too old …
```

Fresco 已经会优先选用 `PATH` 上较新的 `mpvpaper` 而非较旧的内置版本，所以安装发行版的 `mpvpaper` 包（或用 `scripts/build-mpvpaper.sh` 构建[上游版本](https://github.com/GhostNaN/mpvpaper)）通常就够了。要让 Fresco 使用特定的二进制，把 **`FRESCO_MPVPAPER`** 设为它的完整路径——这会覆盖其他一切选择：

```bash
mkdir -p ~/.config/environment.d
echo 'FRESCO_MPVPAPER=/home/YOU/.local/bin/mpvpaper' > ~/.config/environment.d/fresco.conf
```

注销再登录（systemd 在会话启动时读取 `environment.d`），然后用 `fresco doctor` 确认 `source:` 一行已变为 `FRESCO_MPVPAPER override`。

类似地，**`FRESCO_HWDEC`** 可覆盖 mpv 的硬件解码器（如 `nvdec`、`nvdec-copy`、`vaapi`、`no`）——这是排查 CPU 占用高的诊断开关。默认情况下，检测到 NVIDIA GPU 时 Fresco 选 `nvdec,vaapi,auto-safe`，否则选 `auto-safe`（旋转视频走拷回：`nvdec-copy,auto-copy` / `auto-copy`）。

## 隐私与使用条款

**在你同意之前，什么都不发送。** 首次启动时弹一次同意对话框，之后随时可在设置中更改。

无论选哪边，每天一次，Fresco 记录“有一个安装处于活跃状态”、所在国家与版本号。这是计数。对话框征求同意的是**细节**：

| 你的选择 | Fresco 发送的内容 |
| --- | --- |
| **全部接受** | 计数，外加发行版、桌面环境、会话类型、视频后端、显示器数量、你使用的功能、错误类型、城市与地区，以及每次签到的精确时间。 |
| **拒绝可选项** | 仅计数：一个随机安装 id（绝不从你的硬件或姓名派生）、国家、应用版本、打包方式。你的签到以**日期而非时间**存储。 |

**城市与精确使用时间是可选的**——只有全部接受才会发送，拒绝则绝不发送。任何层级都绝不收集坐标：地理定位端点会丢弃经纬度而不是返回它们。

这些数字用于决定发布前要测试哪些发行版和桌面环境，以及哪些地区的下载需要镜像。它们绝不出售、绝不共享、绝不用于广告，也没有任何分析服务商介入。

**无论选哪边都绝不收集：** 个人数据、文件名、你的壁纸、IP 地址、音频、按键或剪贴板内容。国家由 Cloudflare 在网络边缘从你的 IP 解析，所以本项目只会收到一个两字母代码。（你主动写下并按下发送的文字——一条反馈评论，或发给下文维护者的消息——是你选择发送的消息，不属于收集。）

要完全不发送任何东西，在 `~/.config/fresco/config.toml` 里设 `telemetry = false` 和 `telemetry_prompted = false`。

### 与维护者匿名交流

**菜单 → 给维护者留言** 会开启与 Fresco 维护者的一条私密双向对话。无需账号、无需邮箱、无需 GitHub 登录。它是双向匿名的：除了“维护者”你不会知道对方是谁，对方也不知道你是谁——只能看到你的消息，以及（如果你保留勾选）对话框中展示的配置摘要。

对话由一个随机凭据标识，与遥测分开生成、存在不同文件里，因此对话永远无法与使用画像关联。无论你全部接受还是拒绝可选项，它的行为完全一致。在你发出第一条消息之前，什么都不存在。

📄 **[完整使用条款与隐私政策 →](TERMS.md)**——每个字段，逐项列表，毫无遗漏。每一行发送数据的代码都在 [`src/telemetry.rs`](src/telemetry.rs) 里，你可以查证而不是选择相信。

## 项目信息

| | |
|---|---|
| **名称** | Fresco（应用 ID `io.github.dibbayajyotiroy.Fresco`） |
| **软件类型** | Linux 桌面的动态壁纸与视频壁纸应用 |
| **许可证** | [GPL-3.0-or-later](LICENSE)——免费开源，无广告，无需账号 |
| **语言与技术栈** | Rust、GTK4 / libadwaita、libmpv；Wayland 上内置 mpvpaper 渲染器 |
| **首次发布** | v0.0.1，2026 年 6 月 |
| **最新版本** | 1.1.53（2026-10-28）——最新版本见 [Releases](https://github.com/DibbayajyotiRoy/fresco/releases/latest) |
| **安装包** | `.deb`（amd64）、一行安装脚本、Deepin 应用商店；其他发行版从源码构建 |
| **界面语言** | 英文，另有 13 种翻译：简体中文、俄语、西班牙语、法语、德语、巴西葡萄牙语、土耳其语、越南语、印尼语、印地语、孟加拉语、罗马尼亚语和阿拉伯语 |
| **用户规模** | 来自 128 个国家/地区的 3,000+ 名用户 |
| **维护者** | Dibbayajyoti Roy |
| **源代码** | <https://github.com/DibbayajyotiRoy/fresco> |
| **官网** | <https://fresco.dibbayajyoti.com> |

## 贡献与反馈

欢迎 bug 报告、功能建议和 PR——开一个 [issue](https://github.com/DibbayajyotiRoy/fresco/issues)，或使用应用内的反馈对话框。参见 [CONTRIBUTING.md](CONTRIBUTING.md)。

## 许可证

[GPL-3.0-or-later](LICENSE)——免费且开源。

---

<sub>Fresco 是一款免费开源的 Linux 动态壁纸与视频壁纸软件（支持 X11 与 Wayland），带有绘制进壁纸的歌词、时钟、音频可视化和专辑封面挂件。它是原生 Linux 平台上 Wallpaper Engine 的替代品，也可作为 Conky 之类工具在音乐与时间展示上的补充。已有 128 个国家/地区的 3,000+ 人在使用。最后更新：2026-10-28。</sub>
