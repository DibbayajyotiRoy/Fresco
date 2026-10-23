<div align="center">

<img src="./data/icons/hicolor/256x256/apps/io.github.dibbayajyotiroy.Fresco.png" width="112" alt="Ícone do aplicativo Fresco — um app gratuito e de código aberto de papel de parede animado para Linux" />

# Fresco — app gratuito de papel de parede animado e em vídeo para Linux

[English](README.md) | [বাংলা](README.bn.md) | [Deutsch](README.de.md) | [Español](README.es.md) | [Français](README.fr.md) | [हिन्दी](README.hi.md) | [Bahasa Indonesia](README.id.md) | **Português (Brasil)** | [Română](README.ro.md) | [Русский](README.ru.md) | [Türkçe](README.tr.md) | [Tiếng Việt](README.vi.md) | [简体中文](README.zh-CN.md)

**O Fresco é um app gratuito e de código aberto de papel de parede animado (papel de parede ao vivo) para Linux: ele define qualquer vídeo, GIF ou imagem como plano de fundo animado da área de trabalho, no X11 e no Wayland.** É uma alternativa nativa ao Wallpaper Engine para Linux, com interface gráfica, reprodução acelerada por GPU e widgets de desktop.

[![Última versão do Fresco no GitHub](https://img.shields.io/github/v/release/DibbayajyotiRoy/fresco?style=flat-square&label=release)](https://github.com/DibbayajyotiRoy/fresco/releases/latest)
[![Licença GPL-3.0-or-later](https://img.shields.io/github/license/DibbayajyotiRoy/fresco?style=flat-square)](LICENSE)
[![Status do workflow de publicação](https://img.shields.io/github/actions/workflow/status/DibbayajyotiRoy/fresco/publish.yml?style=flat-square&label=publish)](https://github.com/DibbayajyotiRoy/fresco/actions/workflows/publish.yml)
[![Estrelas no GitHub](https://img.shields.io/github/stars/DibbayajyotiRoy/fresco?style=flat-square)](https://github.com/DibbayajyotiRoy/fresco/stargazers)

[Site](https://fresco.dibbayajyoti.com) · [Instalação](#instalação) · [Ambientes compatíveis](#ambientes-compatíveis) · [Comparação](#fresco-vs-wallpaper-engine-hidamari-komorebi-e-mpvpaper) · [Perguntas frequentes](#perguntas-frequentes-faq) · [Widgets](#widgets-de-desktop-letras-relógios-visualizador-e-capa-do-álbum) · [Histórico de alterações](CHANGELOG.md) · [Issues](https://github.com/DibbayajyotiRoy/fresco/issues)

<img src="./data/screenshots/gallery.png" alt="A biblioteca de papéis de parede do Fresco em uma área de trabalho Linux escura: uma grade pesquisável de papéis de parede em vídeo, imagem e apresentação de slides, com um vídeo marcado como ativo e um indicador de status mostrando decodificação por hardware NVDEC" width="800" />

</div>

## O que é o Fresco?

O Fresco transforma um vídeo, GIF, imagem, apresentação de slides ou playlist no papel de parede atrás das suas janelas, por meio de um app GTK4 / libadwaita que dispensa o terminal. A reprodução passa pelo mpv, então a decodificação acontece na GPU e o uso de CPU fica perto de zero — veja [Desempenho](#desempenho-e-duração-da-bateria) para saber quanto isso custa na tomada. Seu papel de parede é restaurado a cada login.

- **Reproduz:** vídeos (mp4, webm, mkv, avi, mov), GIFs animados, imagens, apresentações de slides de imagens e playlists de vídeo.
- **Desktops:** sessões X11 (GNOME no Xorg, Cinnamon, Xfce, MATE, Deepin 25) e Wayland (COSMIC, Hyprland, Sway, KDE Plasma 6, Cinnamon 6.8+). O GNOME no Wayland mostra apenas um quadro estático — veja [Ambientes compatíveis](#ambientes-compatíveis).
- **Decodificação por hardware:** VA-API (Intel, AMD) e NVDEC (NVIDIA), via mpv.
- **Biblioteca:** vincule uma pasta de papéis de parede que você já tem em vez de importá-la e mantenha-a sincronizada conforme os arquivos entram e saem. A biblioteca desenha só o que está na tela, então milhares de papéis de parede continuam rápidos e leves.
- **Widgets:** letras sincronizadas com a música, relógios (incluindo um relógio mundial), um visualizador de áudio e capa do álbum, desenhados dentro do papel de parede. Todos desativados por padrão.
- **Controle:** o app, uma paleta de comandos com Ctrl+K e `fresco next` / `fresco prev` / `fresco random` para atalhos de teclado.
- **Instalação:** um `.deb` para Ubuntu, Debian, Linux Mint, Pop!_OS, elementary OS, Kali e Deepin (script de uma linha, download manual ou a loja de aplicativos do Deepin); compilação a partir do código-fonte no Fedora, Arch e outras — veja [Instalação](#instalação).
- **Licença:** gratuito e de código aberto sob a [GPL-3.0-or-later](LICENSE), sem anúncios e sem conta.
- **Usuários:** mais de 3.000 pessoas em 128 países. Versão mais recente: 1.1.51.

## Instalação

O Fresco é empacotado como `.deb` para as distribuições da família Debian e, nas demais, é compilado a partir do código-fonte.

### Ubuntu, Debian, Linux Mint, Pop!_OS, elementary OS e Kali Linux

Instalação em uma linha — baixa o `.deb` mais recente do GitHub Releases e o instala com o apt:

```bash
curl -fsSL https://github.com/DibbayajyotiRoy/fresco/releases/latest/download/install.sh | FRESCO_SOURCE=github bash
```

Ou baixe o `.deb` em [Releases](https://github.com/DibbayajyotiRoy/fresco/releases/latest) e execute:

```bash
sudo apt install ./fresco_*.deb
```

### Deepin 25

Abra a **Loja de Aplicativos**, procure por **Fresco** e clique em **Instalar**. O Fresco está publicado na Loja de Aplicativos da comunidade deepin, então não há nada para baixar manualmente e as atualizações chegam pela própria loja. A instalação em uma linha acima também funciona no Deepin.

### Fedora, Arch Linux e outras distribuições

Ainda não há pacote pré-compilado para elas, então compile a partir do código-fonte. Você precisa do Rust (via [rustup](https://rustup.rs)) e dos cabeçalhos de desenvolvimento do GTK4, do libadwaita e do libmpv:

```bash
git clone https://github.com/DibbayajyotiRoy/fresco.git && cd fresco
cargo build --release --all-features
```

Em compositores Wayland, o Fresco também precisa do renderizador `mpvpaper`, que o `.deb` já inclui e a compilação a partir do código-fonte não:

```bash
# Arch Linux, CachyOS, EndeavourOS (AUR)
yay -S mpvpaper

# Fedora e qualquer outra: compile-o contra o libmpv do seu sistema
scripts/build-mpvpaper.sh
install -Dm755 target/release/mpvpaper ~/.local/bin/mpvpaper   # precisa estar no PATH
```

O Fresco não está no AUR nem no Flathub — o pacote do AUR chamado `fresco` é um programa de física sem relação com ele. Detalhes e solução de problemas estão em [docs/INSTALL.md](docs/INSTALL.md).

Depois de instalar, abra o **Fresco** pelo lançador de aplicativos. Se algo parecer errado, execute `fresco doctor`.

## Como definir um vídeo como papel de parede no Linux

Instale o Fresco, clique em **Adicionar**, escolha o seu vídeo e clique em **Definir como papel de parede**. O vídeo toca atrás das suas janelas e volta automaticamente no login.

1. Instale o Fresco (acima) e abra-o pelo lançador de aplicativos.
2. Clique em **Adicionar** e escolha um vídeo, GIF ou imagem — ou cole um link com **Por link**.
3. Se quiser, recorte ou gire no editor.
4. Clique em **Definir como papel de parede** e feche o app.

O papel de parede continua tocando depois que a janela é fechada.

## Recursos

- **Qualquer mídia** — vídeo em loop (mp4/webm/mkv), GIF animado, imagem estática, apresentação de slides de imagens, playlist com vários vídeos
- **Adicionar por link** — cole um pin do Pinterest ou qualquer URL direta de vídeo/imagem; o Fresco baixa o arquivo e abre o editor de recorte
- **Vincular pasta** — use uma pasta de papéis de parede que você já tem (imagens, GIFs e vídeos) sem importá-la; ela continua sincronizada quando você adiciona e remove arquivos, as subpastas são opcionais e seus favoritos e edições são mantidos por arquivo
- **Bibliotecas grandes** — a biblioteca só desenha os papéis de parede perto da sua posição de rolagem, então milhares deles abrem em cerca de dois segundos e usam algumas centenas de megabytes (numa biblioteca de teste com 4.000 papéis de parede: cerca de 2 s e 325 MB, contra 9 a 13 s e 1,6 GB)
- **Decodificação por hardware** — a decodificação de vídeo na GPU (VA-API / NVDEC) mantém o uso de CPU perto de zero ao levar o trabalho para o mecanismo de vídeo, onde ele é mensuravelmente mais barato (mas não de graça — veja [Desempenho](#desempenho-e-duração-da-bateria))
- **Economia de energia** — escalonamento de GPU mais leve para notebooks; medido em cerca de metade do consumo da GPU (veja [Desempenho](#desempenho-e-duração-da-bateria))
- **Vários monitores** — um papel de parede diferente em cada tela, com reprodução sincronizada quando o mesmo vídeo é usado em vários monitores
- **Agendamentos de dia e noite** — troque de papel de parede por temporizador, por faixas de horário livres ou pelo nascer e pôr do sol
- **Widgets de desktop** — letras de músicas sincronizadas, relógios com temas, um relógio mundial, um visualizador de áudio e um disco de capa do álbum girando, desenhados no próprio papel de parede para que nada cubra as suas janelas (o relógio mundial tem um modo opcional que flutua sobre as janelas); todos desativados por padrão (veja [Widgets de desktop](#widgets-de-desktop-letras-relógios-visualizador-e-capa-do-álbum))
- **Tela de bloqueio** — mostre o seu papel de parede e os widgets (relógio, saudação, música tocando, bateria e mais) na sua tela de bloqueio real, no lugar da padrão — vídeo ao vivo e widgets no COSMIC 1.9+ e no KDE Plasma 6, papel de parede e widgets via `fresco lock` no Sway/Hyprland/niri e no X11, e um quadro estático nos demais. Desativada por padrão, e o Fresco nunca vê a sua senha — veja [docs/LOCKSCREEN.md](docs/LOCKSCREEN.md)
- **Controle pelo teclado** — `fresco next`, `fresco prev` e `fresco random` para atalhos do compositor (uma janela aberta do Fresco acompanha a troca em vez de desfazê-la), e uma paleta de comandos com Ctrl+K dentro do app
- **Gerenciamento em lote** — selecione vários papéis de parede de uma vez e remova-os em uma só etapa
- **Catálogo integrado** — navegue pelo app por papéis de parede selecionados e com licenças adequadas
- **Pausa automática em tela cheia** — por monitor, no X11, em compositores wlroots, no COSMIC e no KDE Plasma (por meio do plugin de papel de parede dele); opcionalmente também para janelas maximizadas, além de pausar na bateria
- **Extensão de nova aba do navegador** — espelhe o seu papel de parede em cada nova aba (Chrome/Brave/Edge/Firefox; carregue como extensão descompactada a partir de [`./extension`](extension))
- **Suporte ao Deepin DDE** — no Deepin 25, o Fresco se adapta sozinho ao desktop DDE, e um clique na área de trabalho traz os ícones de volta por dez segundos sempre que você precisar deles (veja [Solução de problemas](#os-ícones-da-área-de-trabalho-ficam-ocultos-enquanto-o-papel-de-parede-toca-no-deepin))
- **Editor de recorte e rotação**, som e volume por papel de parede, transições de apresentação de slides e uma biblioteca com busca

## Ambientes compatíveis

| Ambiente | Papel de parede animado | Observações |
|---|---|---|
| X11 (GNOME, Cinnamon, XFCE, …) | ✅ | Renderizador embutido. No GNOME, apenas onde a distribuição ainda oferece uma sessão Xorg (veja a nota sobre o GNOME abaixo) |
| Kali Linux (Xfce, X11) | ✅ | Funciona na sessão padrão do Kali, Xfce sobre X11, com o mesmo renderizador embutido dos outros desktops X11 |
| Xfce (X11) | ✅ | Os ícones da área de trabalho continuam visíveis sobre o papel de parede, e você pode clicar, selecionar e arrastá-los |
| MATE (X11) | ✅ | Os ícones da área de trabalho continuam visíveis e clicáveis sobre o papel de parede |
| Deepin 25 (DDE, X11) | ✅ | Adaptação automática ao DDE — verificado pela comunidade no Deepin 25 Community build1 |
| COSMIC (Wayland) | ✅ | layer-shell |
| Hyprland | ✅ | layer-shell |
| Sway | ✅ | layer-shell |
| Cinnamon 6.8 e mais recentes (Wayland) | ✅ | O compositor do Cinnamon ganhou layer-shell na 6.8. O Cinnamon 6.6 e anteriores não têm layer-shell e ficam com um quadro estático no Wayland |
| KDE Plasma 6 (X11 e Wayland) | ✅ | Definido pelo plugin de papel de parede do próprio plasmashell, então os ícones da área de trabalho continuam visíveis. Para vídeo, precisa do `qml6-module-qtmultimedia`. A reprodução é a do Qt, não a do mpv: sem som, sem ajuste de hwdec, recorte ou transições; uma playlist toca só o primeiro arquivo e uma apresentação de slides mostra só o primeiro quadro. A opção Ajuste (Cobrir, Ajustar, Esticar) é respeitada |
| GNOME no Wayland | ⚠️ | Apenas quadro estático — o Mutter não expõe nenhuma superfície de papel de parede animado, então não há vídeo nem widgets. Vídeo ao vivo depende de uma extensão do Fresco para o GNOME (planejada) |

Todos os ambientes acima são exercitados em modo headless na CI a cada versão.

**Nota sobre o GNOME.** O GNOME no Wayland é o único desktop popular em que o Fresco só consegue mostrar um quadro estático. O antigo jeito de contornar isso, entrar em uma sessão Xorg, está acabando: o GNOME 49 desativou a sessão X11 e o GNOME 50 a removeu, então o Ubuntu 25.10 e posteriores (incluindo o 26.04 LTS) e o Fedora 43 e posteriores não trazem nenhuma sessão Xorg para alternar. O Ubuntu 22.04 e o 24.04 ainda oferecem "Ubuntu on Xorg". Onde não há sessão Xorg, o vídeo ao vivo no GNOME padrão precisa esperar por uma extensão do Fresco para o GNOME, que está planejada; o Fresco avisa isso no app em vez de fingir que o vídeo está tocando. KDE Plasma, COSMIC, Hyprland, Sway e todos os desktops X11 não são afetados.

> "Fácil de usar e com interface limpa — um dos poucos apps de papel de parede animado realmente adaptados ao Deepin 25, instalável por .deb e rodando sem engasgos com reprodução acelerada por hardware."
>
> — 柒玖 (fórum do deepin) / 柒仈玖 (GitHub), testado no Deepin 25 Community build1, sessão X11, Intel Alder Lake-N [Intel Graphics]

O Deepin 25 traz o X11 como sessão padrão, e é nessa sessão que o Fresco foi verificado ali. O compositor Wayland do próprio Deepin, o [Treeland](https://github.com/linuxdeepin/treeland), ainda está em desenvolvimento, então o Fresco não faz nenhuma afirmação sobre o Deepin no Wayland por enquanto.

## Fresco vs Wallpaper Engine, Hidamari, Komorebi e mpvpaper

| Ferramenta | Plataforma | Wayland | X11 | Interface | Reproduz | Licença |
|---|---|---|---|---|---|---|
| **Fresco** | Linux | ✅ compositores layer-shell e KDE Plasma 6; quadro estático no GNOME | ✅ | App GTK4, mais uma CLI `fresco` | Vídeo, GIF, imagens, apresentações de slides, playlists | GPL-3.0-or-later |
| **Wallpaper Engine** | Windows (Steam); sem versão oficial para Linux | n/d | n/d | App gráfico | Papéis de parede de vídeo, de cena e da web | Pago, código fechado |
| **[linux-wallpaperengine](https://github.com/Almamu/linux-wallpaperengine)** | Linux; precisa dos recursos de uma cópia do Wallpaper Engine na Steam | ✅ compositores com layer-shell | ⚠️ não quando um ambiente de desktop (GNOME, KDE) desenha o plano de fundo | Linha de comando; o README dele lista GUIs separadas feitas pela comunidade | Papéis de parede do Wallpaper Engine | GPL-3.0 |
| **[Hidamari](https://github.com/jeffshee/hidamari)** | Linux, voltado ao GNOME | ⚠️ GNOME no Wayland | ✅ | App gráfico | Vídeo | GPL-3.0 |
| **[Komorebi](https://github.com/cheesecakeufo/komorebi)** | Linux | — | — | App gráfico | Papéis de parede de imagem, vídeo e página da web | GPL-3.0 |
| **[mpvpaper](https://github.com/GhostNaN/mpvpaper)** | Linux, compositores baseados em wlroots | ✅ layer-shell | ❌ | Linha de comando | Vídeo, via mpv | GPL-3.0 |
| **xwinwrap** | Linux, somente X11 | ❌ | ✅ | Linha de comando | O player que você acoplar (mpv, gifview) | — |
| **swww** (agora [awww](https://codeberg.org/LGFae/awww)) | Linux, compositores layer-shell; não o GNOME | ✅ | ❌ | Daemon de linha de comando | GIFs animados e imagens, não vídeo | GPL-3.0 |

O Fresco inclui o `mpvpaper` como renderizador para Wayland, então ele se apoia nesse projeto em vez de competir com ele. Um `—` significa que a documentação do próprio projeto não deixa isso claro. Conferido no repositório público de cada projeto em 2026-10-16: o último push no repositório do Komorebi foi em julho de 2025, e o repositório do swww no GitHub está arquivado porque o projeto migrou para o Codeberg com o nome awww.

## Desempenho e duração da bateria

柒玖 (fórum do deepin) / 柒仈玖 (GitHub) mediu a potência do pacote com o `turbostat` em um Intel N150 (Deepin 25, VA-API, duas execuções por nível) enquanto um papel de parede em vídeo tocava:

| Vídeo | Economia de energia | Consumo da GPU | Consumo total do pacote |
|---|---|---|---|
| 1080p 60fps | Qualidade máxima | 1,37 W | 6,00 W |
| 1080p 60fps | **Reduzida** (padrão) | **0,63 W** (−54%) | **4,03 W** (−33%) |
| 4K 60fps | Qualidade máxima | 2,77 W | 7,94 W |
| 4K 60fps | **Reduzida** (padrão) | **1,60 W** (−42%) | **5,95 W** (−25%) |
| 4K 60fps | Mínimo | 0,99 W (−65%) | 4,97 W (−37%) |

A economia de energia reduz o custo de escalonamento da GPU a cada quadro. Nenhum quadro é descartado e a decodificação por hardware não é alterada, então a reprodução continua fluida — o que se perde é a nitidez da imagem, não o movimento. Reduzida é o padrão; vale escolher Mínimo para fontes em 4K.

**Como ler estes números.** A decodificação por hardware não torna um papel de parede animado gratuito — ela transfere o trabalho da CPU para o mecanismo de vídeo, onde é mais barato, mas não inexistente. "Baixo uso de CPU" sozinho é uma afirmação fraca, porque a alternativa com a qual se compara é a decodificação por software, que é muito pior; a medida honesta é o consumo do sistema inteiro, e é por isso que a tabela mostra o **consumo total do pacote** e não só o da GPU.

Duas ressalvas sobre os números acima, para que não sejam lidos como mais do que são:

- São o **consumo total do pacote enquanto um papel de parede toca**, não o custo marginal do papel de parede. Nenhuma medição de repouso foi registrada na mesma máquina, então a diferença entre estes números e a de uma área de trabalho ociosa não está estabelecida aqui. As porcentagens comparam *níveis* de economia de energia entre si, que é para o que elas valem.
- Foi uma só máquina (Intel N150, Alder Lake-N, VA-API, Deepin 25, duas execuções por nível). GPUs dedicadas, NVDEC e outros drivers vão diferir.

Um papel de parede animado sempre custa mais do que um estático. A economia de energia, a pausa automática em tela cheia e a pausa na bateria existem para limitar esse custo, não para fingir que ele é zero.

## Perguntas frequentes (FAQ)

### Existe uma alternativa ao Wallpaper Engine para Linux?

Sim. O Fresco é um app gratuito e de código aberto para Linux que reproduz os seus próprios papéis de parede em vídeo, GIF e imagem por uma interface gráfica, com decodificação por hardware e sem camada de compatibilidade. Ele não abre os arquivos de papel de parede do próprio Wallpaper Engine; se você tem o Wallpaper Engine e quer esses papéis de parede no Linux, o projeto separado [linux-wallpaperengine](https://github.com/Almamu/linux-wallpaperengine) os executa pela linha de comando e precisa dos recursos de uma cópia do Wallpaper Engine na Steam. Veja a [tabela de comparação](#fresco-vs-wallpaper-engine-hidamari-komorebi-e-mpvpaper).

### O Wallpaper Engine funciona no Linux?

Não nativamente. O Wallpaper Engine é um aplicativo do Windows, sem versão oficial para Linux. O Fresco é uma alternativa nativa para Linux, que se instala como `.deb` e dispensa camada de compatibilidade.

### Como definir um vídeo como papel de parede no Ubuntu?

Instale o Fresco, abra-o, clique em **Adicionar**, escolha o seu vídeo e clique em **Definir como papel de parede**. O vídeo toca como plano de fundo da área de trabalho e é restaurado no login. A sessão padrão do Ubuntu, GNOME no Wayland, só consegue mostrar um quadro estático. No Ubuntu 22.04 e no 24.04 você pode entrar em uma sessão **Ubuntu on Xorg** para ter a reprodução animada completa; o Ubuntu 25.10 e posteriores (incluindo o 26.04 LTS) não trazem sessão Xorg, então ali o vídeo ao vivo no GNOME padrão depende de uma extensão do Fresco para o GNOME, que está planejada. Os sabores do Ubuntu que usam KDE Plasma, Xfce ou MATE não são afetados.

### Como ter um papel de parede animado no Ubuntu, Linux Mint, Pop!_OS, Fedora ou Arch Linux?

Instale o `.deb` do Fresco no Ubuntu, no Linux Mint e no Pop!_OS, ou compile-o a partir do código-fonte no Fedora e no Arch Linux. Os comandos de cada um estão em [Instalação](#instalação).

- **Ubuntu:** `.deb`. No desktop GNOME padrão, a reprodução ao vivo exige uma sessão Xorg (Ubuntu 22.04 e 24.04); os sabores com KDE Plasma, Xfce e MATE não são afetados — veja a resposta sobre o Ubuntu acima.
- **Linux Mint:** `.deb`. O Cinnamon no X11 é compatível, e o Cinnamon no Wayland a partir da versão 6.8.
- **Pop!_OS:** `.deb`. O desktop COSMIC (Wayland layer-shell) é compatível.
- **Fedora:** compile a partir do código-fonte. O GNOME do Fedora 43 e posteriores é somente Wayland, então mostra um quadro estático; KDE Plasma e desktops X11 são compatíveis.
- **Arch Linux:** compile a partir do código-fonte e instale o `mpvpaper` pelo AUR (`yay -S mpvpaper`).

Apenas o caminho do `.deb` é coberto pela CI do Fresco.

### Quais distribuições Linux são compatíveis?

O Fresco traz um pacote `.deb` para distribuições baseadas em Debian e Ubuntu: Ubuntu, Pop!_OS, Linux Mint, Debian, elementary OS, Deepin 25 e Kali Linux. As outras distribuições podem compilar a partir do código-fonte — veja [Instalação](#instalação) e [docs/INSTALL.md](docs/INSTALL.md).

### Papéis de parede animados gastam muita CPU, GPU ou bateria?

CPU, não. Bateria, um pouco — um papel de parede animado nunca é de graça.

O Fresco decodifica o vídeo na GPU (VA-API / NVDEC), então o uso de CPU fica perto de zero. Isso move o custo para o mecanismo de vídeo em vez de eliminá-lo. Medido em um Intel N150 no nível padrão de Economia de energia, um papel de parede em 1080p consumiu 0,63 W de GPU, com o consumo total do pacote em 4,03 W durante a reprodução (veja [Desempenho](#desempenho-e-duração-da-bateria) para entender como ler isso, inclusive o que ele *não* diz). A pausa automática em tela cheia e a pausa na bateria existem para manter esse custo longe da bateria quando ele mais importaria.

### O Fresco funciona no Wayland?

Sim, em compositores que implementam o protocolo layer-shell — COSMIC, Hyprland e Sway — e no KDE Plasma 6 (X11 e Wayland) por meio do plugin de papel de parede do próprio plasmashell. O GNOME no Wayland é a exceção: o Mutter não expõe nenhuma superfície de papel de parede, então o Fresco mostra um quadro estático ali e avisa isso no app. As sessões X11 são totalmente compatíveis.

### O Fresco funciona no GNOME?

Sim no X11, com o papel de parede animado completo — vídeo, widgets, tudo. No **GNOME sob Wayland**, o Mutter não expõe nenhuma superfície de papel de parede para um app desenhar, então o Fresco mostra um quadro estático ali e os widgets não ficam disponíveis. Se a sua distribuição ainda oferece uma sessão Xorg (o Ubuntu 22.04 e o 24.04 oferecem), encerre a sessão e escolha "GNOME on Xorg" ou "Ubuntu on Xorg" para ter a reprodução animada completa. O GNOME 49 desativou essa sessão e o GNOME 50 a removeu, então o Ubuntu 25.10 e posteriores (incluindo o 26.04 LTS) e o Fedora 43 e posteriores não a têm: neles, o vídeo ao vivo no GNOME depende de uma extensão do Fresco para o GNOME, que está planejada. O `fresco doctor` diz em qual caso você está.

### O Fresco funciona no KDE Plasma, COSMIC, Hyprland ou Sway?

Sim, nos quatro. COSMIC, Hyprland e Sway reproduzem vídeo ao vivo por layer-shell. O KDE Plasma 6 (X11 e Wayland) usa o plugin de papel de parede do próprio Fresco dentro do plasmashell, que mantém os ícones da área de trabalho visíveis, mas reproduz pelo Qt em vez do mpv: sem som, sem recorte nem transições, e precisa do `qml6-module-qtmultimedia` para vídeo. A opção Ajuste (Cobrir, Ajustar ou Esticar) também vale ali.

### O Fresco funciona no Linux Mint, Xfce, Cinnamon ou MATE?

Sim. Cinnamon, Xfce e MATE no X11 usam o renderizador embutido, e os ícones da área de trabalho continuam visíveis sobre o papel de parede no Xfce e no MATE; no Xfce, você pode clicar, selecionar e arrastá-los normalmente. O Cinnamon no Wayland funciona a partir do Cinnamon 6.8, que adicionou layer-shell.

### Como trocar de papel de parede com um atalho de teclado?

Associe `fresco next`, `fresco prev` ou `fresco random` a uma tecla nas configurações do seu compositor ou desktop — eles trocam o papel de parede sem abrir o app (o `random` nunca escolhe o que já está tocando). Eles percorrem a sua biblioteca na ordem em que o app a mostra, ou apenas a pasta que você tem aberta ali, e iniciam o serviço do Fresco se ele não estiver em execução. Se a janela do Fresco estiver aberta nessa hora, ela acompanha o novo papel de parede em vez de recolocar o antigo. Dentro do app, Ctrl+K abre uma paleta de comandos. Por exemplo, no Sway:

```
bindsym $mod+Right  exec fresco next
bindsym $mod+Left   exec fresco prev
bindsym $mod+Return exec fresco random
```

### Posso usar um papel de parede diferente em cada monitor?

Sim. O Fresco aceita papéis de parede por tela e, quando o mesmo vídeo é usado em vários monitores, a reprodução é mantida em sincronia.

### Ele aceita GIFs e apresentações de slides de imagens?

Sim — GIFs animados, imagens estáticas, apresentações de slides de imagens com transições (esmaecer para preto, Ken Burns, zoom e desfoque) e playlists com vários vídeos, além dos arquivos de vídeo.

### O Fresco pode usar minha pasta de papéis de parede sem importar?

Sim. Escolha **Vincular pasta…** no menu (ou **Vincular esta pasta** depois de **Adicionar pasta**) para apontar o Fresco para uma pasta onde você já guarda papéis de parede. As imagens, GIFs e vídeos dela aparecem na sua biblioteca, numa seção própria, onde estão e sem serem importados, e a biblioteca continua sincronizada conforme você adiciona e remove arquivos nessa pasta. **Incluir subpastas** é opcional (até quatro níveis de profundidade), e seus favoritos e edições são lembrados por arquivo. Se a pasta sumir, por exemplo por causa de um disco desconectado, a seção dela continua marcada como “(indisponível)” e volta sozinha. **Desvincular pasta**, no menu da seção, remove a seção sem apagar nenhum arquivo. **Adicionar** continua importando arquivos individuais como antes.

### O Fresco aguenta milhares de papéis de parede?

Sim. A biblioteca só monta os cartões dos papéis de parede perto de onde você está rolando, em vez de todos de uma vez, então uma coleção grande não pesa no app. Numa biblioteca de teste com 4.000 papéis de parede (metade imagens, metade vídeos), a biblioteca apareceu em cerca de 2 segundos em vez de 9 a 13, e o app usou aproximadamente 325 MB de memória em vez de 1,6 GB. Esses números vêm de uma versão de desenvolvimento sob carga em uma única máquina, então encare-os como uma referência, não como uma promessa.

### Posso usar um vídeo do YouTube como papel de parede?

Não colando o link dele. **Por link** aceita pins do Pinterest e links diretos para um arquivo de vídeo ou imagem (mp4, webm, mkv, mov, gif, jpg, png, webp); outros links são rejeitados com uma mensagem. Adicione com **Adicionar** um arquivo de vídeo que você já tenha, ou cole um link direto para o arquivo.

### O Fresco é gratuito e de código aberto?

Sim. O Fresco é gratuito e de código aberto sob a GPL-3.0-or-later. Não há anúncios, contas nem plano pago.

### O Fresco coleta meus dados?

Nada é enviado até você responder a uma caixa de diálogo de consentimento, que aparece uma única vez. Você escolhe entre uma contagem diária anônima (ID de instalação aleatório, país, versão do app) e essa contagem mais detalhes opcionais; definir `telemetry = false` em `~/.config/fresco/config.toml` não envia absolutamente nada. Veja [Privacidade e termos de uso](#privacidade-e-termos-de-uso) para conhecer cada campo.

### Qual é a diferença entre o Fresco e o mpvpaper?

O `mpvpaper` é um wrapper de linha de comando do mpv que toca um único vídeo como plano de fundo layer-shell do Wayland — sem GUI, sem biblioteca, sem suporte a X11. O Fresco é um app de desktop GTK4 construído em torno dele: biblioteca de papéis de parede, editor de recorte e rotação, agendamento, sincronização entre vários monitores, widgets de desktop e suporte a X11, sem precisar de terminal. O Fresco inclui o `mpvpaper` como renderizador para Wayland em vez de substituí-lo — veja a [tabela de comparação](#fresco-vs-wallpaper-engine-hidamari-komorebi-e-mpvpaper).

### Posso usar o Fresco no Kali Linux?

Sim. O Kali Linux é baseado em Debian, então o Fresco se instala do mesmo jeito que no Debian ou no Ubuntu — com o pacote `.deb` ou com o script de instalação em uma linha (veja [Instalação](#instalação)). O desktop padrão do Kali, Xfce sobre X11, é compatível por meio do renderizador X11 embutido do Fresco.

### Como remover vários papéis de parede de uma vez?

Clique em **Selecionar** no rodapé (ou clique com o botão direito em um papel de parede e escolha **Selecionar…**), marque os que quiser e clique em **Remover**. **Selecionar tudo** respeita a busca atual, então você pode pesquisar primeiro e depois limpar um lote inteiro. Remover um papel de parede o tira da biblioteca do Fresco — o arquivo de origem no disco é mantido.

### Como desinstalar o Fresco?

Se você instalou o `.deb` ou usou o script de instalação, execute `sudo apt remove fresco`. Sua biblioteca e sua configuração ficam em `~/.local/share/fresco/` e `~/.config/fresco/`; apague esses diretórios para eliminar qualquer vestígio. Para recuperar o papel de parede do próprio desktop sem desinstalar, abra o Fresco e clique em **Parar papel de parede**.

## Widgets de desktop: letras, relógios, visualizador e capa do álbum

O Fresco desenha cinco widgets opcionais sobre o seu papel de parede. Todos vêm **desativados por padrão**; ative-os pelo menu do app (Ctrl+,) → **Avançado…**.

| Widget | O que você recebe |
|---|---|
| **Letra** | A linha atual, no tempo da música. Quatro predefinições (Minimalista, Karaokê, Legenda, Cartão), uma grade de posicionamento de nove pontos, um controle deslizante de ajuste de sincronia, uma próxima linha esmaecida opcional e título e artista da faixa opcionais |
| **Relógio** | Oito temas — Digital, Minimalista, Segmentado, Empilhado, Detalhado, Cartão, NOS e um visual de tela de bloqueio estilo celular. Formato de 12 ou 24 horas, data opcional, controles de cor e opacidade. Os segundos vêm desativados por padrão porque custam 60× mais redesenhos |
| **Relógio mundial** | A hora em várias cidades ao mesmo tempo, escolhidas na lista completa de fusos horários, em texto simples de 12 horas (por exemplo `Tokyo  9:41 PM`), com um rótulo opcional por cidade. Redesenha uma vez por minuto. Novo na 1.1.50 |
| **Visualizador de áudio** | Cinco estilos — Barras, Espelhar, Onda, Pontos, Anel — com seletor de cor, mesclagem de duas cores ou arco-íris |
| **Capa do álbum** | A capa da faixa atual em um disco girando; ele para de girar quando a reprodução é pausada |

Exceto pelo modo opcional **Flutuar sobre as janelas** do relógio mundial, os widgets são pintados no papel de parede pela camada de OSD do mpv, e não em uma janela própria, então nunca ficam acima das suas janelas, nunca interceptam um clique e se comportam de forma idêntica no X11 e em todo compositor layer-shell. Com música tocando e os widgets de letra, relógio, visualizador e capa do álbum ativados, o custo medido foi de **0,8% de um núcleo de CPU** — quase tudo da captura de áudio, já que nada é redesenhado se o conteúdo não mudou. Os widgets também voltam por conta própria depois que a máquina é suspensa e retomada.

Os widgets aparecem em **todas as telas** por padrão. Para mantê-los em uma só, adicione `monitor = "DP-1"` ao bloco `[widgets]` do `config.toml` (ainda não há controle na interface gráfica para isso). Eles **não estão disponíveis no GNOME sob Wayland**, que não tem superfície de papel de parede animado para o Fresco desenhar — o mesmo motivo pelo qual os papéis de parede voltam a um quadro estático ali.

### Posso colocar um relógio no papel de parede da minha área de trabalho Linux?

Sim. Ative o widget **Relógio** para um relógio com tema desenhado no papel de parede, ou o **Relógio mundial** para mostrar várias cidades ao mesmo tempo. O relógio mundial fica no papel de parede como os outros widgets; o modo opcional **Flutuar sobre as janelas** o ergue acima de todos os apps, deixando os cliques passarem, o que funciona no X11 com um compositor e em compositores layer-shell (KDE Plasma, COSMIC, Sway, Hyprland), e volta para o papel de parede onde a sessão não consegue fazê-lo flutuar, como o GNOME no Wayland.

### Posso mostrar letras de músicas na minha área de trabalho Linux?

Sim. O Fresco desenha **letras sincronizadas com o tempo** sobre o seu papel de parede, acompanhando o que estiver tocando no seu sistema via MPRIS — navegadores, apps de música, players de vídeo. Escolha uma predefinição, o posicionamento e o ajuste de sincronia nas configurações do widget Letra (veja a tabela acima).

### O Linux tem widgets de desktop como o Conky?

Sim, e o Fresco acrescenta cinco que não precisam de painel, de extensão nem de suporte do seu desktop: um relógio, um relógio mundial, letras sincronizadas, um visualizador de áudio e um disco de capa do álbum girando. Como são pintados no papel de parede em vez de em uma janela, funcionam em desktops que não têm camada de widgets própria — inclusive COSMIC, Hyprland e Sway. Ao contrário do Conky, o Fresco **não tem widgets de monitoramento do sistema** (nada de leituras de CPU, RAM, temperatura ou rede), então ele convive com o Conky como um companheiro de música e hora, e não o substitui. O GNOME no Wayland é o único lugar em que ele não pode rodar.

### Posso ter um visualizador de música no plano de fundo da área de trabalho?

Sim. O visualizador de áudio do Fresco reage a tudo o que o seu sistema estiver tocando, em um de cinco estilos — Barras, Espelhar, Onda, Pontos ou Anel — com seletor de cor, mesclagem de duas cores ou arco-íris. Ele vem desativado por padrão e pede consentimento na primeira vez que você o ativa, porque precisa escutar a saída de áudio; o consentimento também é verificado ao carregar a configuração, então editar o `config.toml` à mão não consegue ativá-lo pelas suas costas. Combine-o com o widget de capa do álbum para ter um disco girando com a capa da faixa atual.

### Quais players de música funcionam com o widget de letras?

Qualquer um que publique metadados MPRIS padrão, mas eles não são igualmente confiáveis:

| Player | Funciona? | Observações |
|---|---|---|
| **Firefox** | ✅ | A escolha confiável, e aquilo com que o recurso foi verificado |
| Chrome / Brave / Edge / Vivaldi / Opera | ⚠️ | Veja abaixo |
| Spotify — **no navegador** | ✅ | Informa a posição de reprodução corretamente |
| Spotify — **cliente nativo para Linux** | ⚠️ | Informa a posição como 0 para sempre, então a letra não consegue ficar em sincronia |
| Players locais (VLC, mpv, Rhythmbox, …) | ✅ | Também é onde um arquivo `.lrc` local tem mais chance de existir |

Os navegadores da família Chromium reivindicam um nome MPRIS na primeira vez que
qualquer aba toca mídia e **nunca o liberam** — um bug antigo do Chromium. Quando a
reprodução termina, o título é limpo, mas uma sessão "zumbi" obsoleta continua no
barramento, às vezes ainda com uma capa. O Fresco ignora qualquer player que não
publique título de faixa, o que descarta essas sessões, mas os relatos do próprio
Chromium continuam inconsistentes o bastante para que o Firefox seja o navegador
a usar aqui.

O cliente nativo do Spotify para Linux devolve `Position: 0` e nunca emitiu um
seek desde 2018, tanto nas versões nativas quanto nas snap. O Fresco detecta isso
pelo comportamento — três leituras zero espaçadas durante a reprodução — e passa
a rodar o relógio da letra livremente a partir da troca de faixa, o que se
desalinha se você pular partes da música. O Spotify no navegador não tem esse
problema.

### De onde vêm as letras?

Primeiro, de um arquivo `.lrc` local: um que esteja ao lado do arquivo de áudio, ou
um correspondente em uma pasta de letras que você indicar ao Fresco. Esse caminho
funciona offline e é a melhor correspondência, porque é o arquivo que você escolheu.

Quando não há arquivo local — o que acontece na maior parte do tempo se você usa
streaming —, o Fresco procura a faixa no [LRCLIB](https://lrclib.net), um banco
de dados gratuito de letras sincronizadas mantido pela comunidade, e guarda a
resposta em cache em `~/.cache/fresco/lyrics` para que a mesma música nunca seja
buscada duas vezes. Essa consulta envia ao LRCLIB o título, o artista e o álbum
da faixa; nada é enviado a menos que o widget de letras esteja ativado e a faixa
não tenha arquivo local. O Fresco não hospeda, não é dono nem licencia conteúdo
de letras — as entradas do LRCLIB são enviadas por seus usuários e o LRCLIB não
declara nenhuma licença sobre elas, então o Fresco busca sob demanda e guarda em
cache por usuário, em vez de distribuir um banco de dados de letras próprio.

A marcação de tempo das letras em arquivos `.lrc` é feita à mão e muitas vezes
está um pouco fora. Há um controle deslizante de ajuste de sincronia nas
configurações de letra justamente para isso.

## Solução de problemas

Execute primeiro o `fresco doctor`: ele informa a sua sessão, qual renderizador o Fresco escolheu e de onde ele veio, e se o contorno do Xorg no GNOME se aplica. O [docs/INSTALL.md](docs/INSTALL.md) cobre mais casos, inclusive uso alto de CPU. Inclua a saída do `fresco doctor` ao abrir uma issue.

Ainda travado? [Relate um bug](https://github.com/DibbayajyotiRoy/Fresco/issues/new?template=bug_report.yml) ou [peça um recurso](https://github.com/DibbayajyotiRoy/Fresco/issues/new?template=feature_request.yml).

### O Fresco não aparece no lançador do Deepin depois de instalar — por quê?

Um problema conhecido do dde-launchpad: a atualização que ele faz após a instalação
não detecta o Fresco. Execute `killall dde-shell` (ele reinicia sozinho) ou saia da
sessão e entre de novo, e a entrada aparece de forma permanente. O próprio Fresco
é instalado corretamente — ele aparece no gerenciador de aplicativos do Deepin, e
o ícone é resolvido em todos os temas instalados. Se acontecer a cada reinstalação
em uma máquina que já teve o Fresco antes, execute `sh scripts/dde-launcher-diag.sh`
— ele é somente leitura e imprime um log que vale a pena anexar a uma issue.

### Os ícones da área de trabalho ficam ocultos enquanto o papel de parede toca no Deepin

O Deepin 25 desenha o papel de parede e os ícones da área de trabalho em uma única
janela opaca, então um papel de parede animado que você consiga ver fica em uma
camada acima dessa janela — não há camada que fique entre os dois. **Clique na área
de trabalho e os ícones voltam por dez segundos**, tempo suficiente para abrir o que
você queria; depois disso o papel de parede retorna, e outro clique garante mais dez
segundos. Para mudar o intervalo, defina `dde_icon_peek_secs` em
`~/.config/fresco/config.toml` (`0` mantém o papel de parede sempre por cima). O
papel de parede do próprio Deepin continua aparecendo sempre que o Fresco está
desligado ou em pausa.

### Por que o plano de fundo do meu MATE fica escuro depois de uma falha?

Para manter os ícones da área de trabalho do Caja por cima do papel de parede
animado, o Fresco define o plano de fundo do MATE como uma cor-chave quase preta
(`#010101`) enquanto está em execução e copia para o papel de parede tudo o que não
for essa cor — os ícones. O seu plano de fundo é salvo antes e restaurado quando o
Fresco para ou é desativado. Se o Fresco falhar antes de conseguir fazer isso, a
área de trabalho continua escura; **inicie o Fresco de novo e ele restaura o seu
plano de fundo**. Em um servidor X sem as extensões Composite e Damage, o Fresco
passa a funcionar como no [Deepin](#os-ícones-da-área-de-trabalho-ficam-ocultos-enquanto-o-papel-de-parede-toca-no-deepin):
ícones ocultos, e um clique na área de trabalho os traz de volta.

### Meu papel de parede fica preto no Wayland (NVIDIA, COSMIC, Hyprland, Sway)

Um papel de parede preto — com todo o resto funcionando e nenhum erro no log —
quase sempre significa que o renderizador mpvpaper que o Fresco inclui é antigo
demais para a sua configuração. As versões anteriores à 1.6 inicializam o EGL,
informam sucesso e depois nunca apresentam um quadro no driver proprietário da
NVIDIA.

Execute primeiro o `fresco doctor`. Ele imprime o renderizador escolhido, de onde
veio esse binário (incluído no pacote ou instalado por você) e mais ou menos qual
é a versão, e avisa quando essa versão é anterior à correção:

```
  ✓ mpvpaper available (/usr/lib/fresco/mpvpaper-libmpv2)
      source: bundled · version: 1.4–1.6
  ⚠ mpvpaper may be too old …
```

O Fresco já prefere um `mpvpaper` mais novo no seu `PATH` a um incluído mais
antigo, então instalar o pacote `mpvpaper` da sua distribuição (ou compilar o
[upstream](https://github.com/GhostNaN/mpvpaper) com `scripts/build-mpvpaper.sh`)
costuma bastar. Para apontar o Fresco para um binário específico, defina
**`FRESCO_MPVPAPER`** com o caminho completo dele — isso prevalece sobre todas as
outras escolhas:

```bash
mkdir -p ~/.config/environment.d
echo 'FRESCO_MPVPAPER=/home/YOU/.local/bin/mpvpaper' > ~/.config/environment.d/fresco.conf
```

Saia da sessão e entre de novo (o systemd lê o `environment.d` no início da
sessão), e então confirme com o `fresco doctor` que a linha `source:` agora diz
`FRESCO_MPVPAPER override`.

Da mesma forma, **`FRESCO_HWDEC`** substitui o decodificador de hardware do mpv
(por exemplo `nvdec`, `nvdec-copy`, `vaapi`, `no`) — um recurso de diagnóstico para
uso alto de CPU. Por padrão, o Fresco escolhe `nvdec,vaapi,auto-safe` quando há uma
GPU NVIDIA, e `auto-safe` nos demais casos (vídeo girado usa copy-back:
`nvdec-copy,auto-copy` / `auto-copy`).

## Privacidade e termos de uso

**Nada é enviado até você responder.** Uma caixa de diálogo de consentimento pergunta uma vez, na primeira execução, e a escolha pode ser alterada a qualquer momento nas configurações.

De qualquer forma, uma vez por dia o Fresco registra que uma instalação esteve ativa, em qual país e em qual versão. Essa é a contagem. O que a caixa de diálogo pergunta são os **detalhes**:

| Você escolhe | O que o Fresco envia |
| --- | --- |
| **Aceitar tudo** | A contagem, mais distribuição, desktop, tipo de sessão, backend de vídeo, número de monitores, quais recursos você usa, tipos de erro, cidade e região e o horário exato de cada verificação. |
| **Recusar opcionais** | Somente a contagem: um ID de instalação aleatório (nunca derivado do seu hardware ou nome), país, versão do app, empacotamento. Sua verificação é guardada como **data, não como horário**. |

**A cidade e o horário exato de uso são opcionais** — enviados somente se você aceitar tudo, nunca enviados se você recusar. Coordenadas nunca são coletadas em nenhum nível: o endpoint de geolocalização descarta latitude e longitude em vez de devolvê-las.

Esses números existem para decidir quais distribuições e desktops são testados antes de uma versão e onde os downloads precisam de um espelho. Eles nunca são vendidos, compartilhados nem usados para publicidade, e não há nenhum fornecedor de análise envolvido.

**Nunca coletados, de nenhuma das duas formas:** dados pessoais, nomes de arquivos, seus papéis de parede, seu endereço IP, áudio, teclas digitadas ou o conteúdo da área de transferência. O país é resolvido a partir do seu IP pela Cloudflare na borda da rede, então só um código de duas letras chega a este projeto. (O texto que você escreve de propósito e envia — um comentário de feedback ou uma mensagem ao mantenedor, abaixo — é uma mensagem que você escolheu enviar, não coleta.)

Para não enviar nada, defina `telemetry = false` e `telemetry_prompted = false` em `~/.config/fresco/config.toml`.

### Fale com o mantenedor, de forma anônima

**Menu → Enviar mensagem ao mantenedor** abre uma conversa privada de mão dupla com a pessoa que faz o Fresco. Sem conta, sem endereço de e-mail, sem login no GitHub. É anônima nos dois sentidos: você não fica sabendo quem é a outra pessoa além de "o mantenedor", e ela não fica sabendo quem é você — apenas as suas mensagens e, se você deixar a caixa marcada, o resumo da configuração mostrado na caixa de diálogo.

A conversa é identificada por um ticket aleatório gerado separadamente da telemetria e guardado em outro arquivo, então uma conversa nunca pode ser associada a um perfil de uso. Funciona exatamente igual se você aceitou tudo ou recusou os opcionais. Nada existe até você enviar a primeira mensagem.

📄 **[Termos de uso e política de privacidade completos →](TERMS.md)** — cada campo, em uma tabela, sem omitir nada. Toda linha que envia alguma coisa está em [`src/telemetry.rs`](src/telemetry.rs), para você poder conferir em vez de confiar.

## Informações do projeto

| | |
|---|---|
| **Nome** | Fresco (ID do aplicativo `io.github.dibbayajyotiroy.Fresco`) |
| **O que é** | Um app de papel de parede animado e em vídeo para a área de trabalho Linux |
| **Licença** | [GPL-3.0-or-later](LICENSE) — gratuito e de código aberto, sem anúncios e sem conta |
| **Linguagem e stack** | Rust, GTK4 / libadwaita, libmpv; um renderizador mpvpaper incluído no Wayland |
| **Primeira versão** | v0.0.1, junho de 2026 |
| **Versão mais recente** | 1.1.51 (2026-10-23) — veja [Releases](https://github.com/DibbayajyotiRoy/fresco/releases/latest) para a mais nova |
| **Pacotes** | `.deb` (amd64), script de instalação em uma linha, Loja de Aplicativos do Deepin; compilação a partir do código-fonte nos demais casos |
| **Idiomas da interface** | Inglês mais 12 traduções: chinês simplificado, russo, espanhol, francês, alemão, português (Brasil), turco, vietnamita, indonésio, hindi, bengali e romeno |
| **Usuários** | Mais de 3.000 pessoas em 128 países |
| **Mantenedor** | Dibbayajyoti Roy |
| **Código-fonte** | <https://github.com/DibbayajyotiRoy/fresco> |
| **Site** | <https://fresco.dibbayajyoti.com> |

## Contribuição e feedback

Relatos de bugs, pedidos de recursos e PRs são bem-vindos — abra uma [issue](https://github.com/DibbayajyotiRoy/fresco/issues) ou use a caixa de diálogo de feedback dentro do app. Veja o [CONTRIBUTING.md](CONTRIBUTING.md).

## Licença

[GPL-3.0-or-later](LICENSE) — gratuito e de código aberto.

---

<sub>O Fresco é um app gratuito e de código aberto de papel de parede animado e papel de parede em vídeo para Linux (X11 e Wayland), com widgets de letras, relógio, visualizador de áudio e capa do álbum desenhados no papel de parede. É uma alternativa nativa ao Wallpaper Engine para Linux e um companheiro de música e relógio para ferramentas como o Conky. Usado por mais de 3.000 pessoas em 128 países. Última atualização: 2026-10-23.</sub>
