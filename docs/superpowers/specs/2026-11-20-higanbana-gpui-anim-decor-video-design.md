# higanbana-gpui: animações, decorations customizáveis e player de vídeo

## Intenção

O higanbana-gpui vai substituir o GPUI no anitsu-client. O app é de arte/anime: precisa
de movimento (UI viva), janela com cara própria (barra de título desenhada pelo app) e
um player de vídeo que hoje existe só no `higanbana-desktop` (mpv em janela filha + GPUI).

Três entregas independentes, nesta ordem (cada uma em branch/commits próprios):
1. Animações  2. Decorations  3. Player de vídeo (usa 1 e 2).

## Não-objetivos

- Física/gestos complexos, animação de layout automática (FLIP) — fica para depois.
- DirectComposition nos backends (descartado).
- Player em janela filha (abordagem B, descartada).
- macOS/Linux: core é Win32; nada novo fora do Windows.

## Anti-objetivos

- Frames contínuos sem animação ativa (CPU ociosa tem que continuar 0%).
- Quebrar qualquer um dos 5 backends.
- Arquivo deus: cada peça em arquivo pequeno por responsabilidade.

## Estado atual

- `src/anim`: `Animator` com tweens `f32` por `ElementId`, `Easing` (linear/in/out/inout/custom),
  `Transition` (duração + curva). `cx.animate` / `cx.animate_from`. App só agenda frame
  enquanto algum tween está ativo. Usado em Switch, Skeleton, overlays.
- Estilo tem `opacity` (aplicada à subárvore), sem transformações.
- `hg_window_desc` = título, tamanho, `resizable`. Barra de título sempre nativa.
- Player existente: `higanbana-desktop/src/video` (libmpv carregada dinamicamente,
  `mpv.rs` já declara a render API).

## 1. Animações

Estender o que existe, sem trocar o modelo (tween consultado no render):

- **Transform de subárvore**: `.translate(x, y)` e `.scale(s)` (origem no centro) no estilo,
  aplicados no paint (quads, texto, imagens, clip) e no hit-test. Sem rotação.
- **Springs**: `Transition::spring(stiffness, damping)` além de duração+curva; retarget
  preserva velocidade.
- **Delay e repetição**: `.delay(ms)`, `.repeat(n | Infinite)`, `.yoyo()` (ida e volta)
  — para spinners, pulse, skeleton.
- **Valores além de f32**: `cx.animate_color`, `cx.animate_rect`/pontos (interpolação
  componente a componente), reaproveitando o mesmo tween.
- **Presença** (entrada/saída): helper que mantém o elemento montado até a animação
  de saída terminar (hoje overlays fazem isso à mão em `overlay/common.rs`; generalizar).
- Animações infinitas mantêm frames só enquanto o elemento está visível na árvore.
- Galeria ganha seção "Animações" com exemplos.

## 2. Decorations customizáveis

`WindowOptions.decorations: Decorations::{Native, Custom}`.

- **Core C** (`window_win32.c`): em `Custom`, janela continua com `WS_THICKFRAME |
  WS_CAPTION` (mantém snap, animação de min/max, sombra), mas `WM_NCCALCSIZE` remove a
  área não-cliente; maximizada compensa a borda invisível.
- **Hit-test**: `WM_NCHITTEST` é síncrono, então o app publica regiões a cada frame:
  `hg_window_set_hit_regions(win, regions, n)` com tipos `CAPTION`, `MIN`, `MAX`, `CLOSE`,
  `CLIENT`. Bordas de redimensionar calculadas no C (largura configurável).
  `HTMAXBUTTON` devolvido no botão maximizar → snap layouts do Windows 11 funcionam.
- **API Rust**: `.window_drag()` (marca elemento como área de arrastar) e
  `.window_control(Minimize | Maximize | Close)`; `cx.window().minimize/maximize/close`,
  `is_maximized()` e evento de mudança.
- **Componente `TitleBar`** (shadcn-style): título, slot para conteúdo (tabs, busca),
  botões min/max/fechar com hover animado; fechar em vermelho.
- Fullscreen: `cx.window().set_fullscreen(bool)` (usado pelo player).

## 3. Player de vídeo (abordagem A, escolhida)

O mpv desenha via render API (OpenGL) num FBO; os pixels voltam para a CPU e viram uma
imagem do higanbana atualizada a cada frame. Funciona igual nos 5 backends.

- **Onde**: módulo `video` no `higanbana-gpui` atrás de feature `video`. Loader da libmpv
  baseado no `higanbana-desktop/src/video/mpv.rs` (mesmo carregamento dinâmico, sem
  dependência em build).
- **Thread de vídeo**: contexto WGL oculto próprio (janela dummy), `mpv_render_context`
  OpenGL, FBO no tamanho do vídeo (limitado ao tamanho exibido), leitura via 2 PBOs
  (assíncrona, sem travar). Frame pronto vai para um buffer duplo compartilhado e acorda
  a UI (`request_redraw`).
- **Core**: API para atualizar pixels de uma imagem existente sem recriar a textura
  (`hg_image_update`) nos 5 backends.
- **Componente `VideoPlayer`**: superfície (`object-fit` contain/cover), controles
  shadcn-style (play/pausa, seek com prévia de tempo, volume, faixas de áudio/legenda,
  velocidade, fullscreen), auto-ocultar com fade (usa 1), duplo clique = fullscreen
  (usa 2), atalhos (espaço, setas, F, M). Estado via eventos do mpv (tempo, duração,
  pausa, fim, erro).
- Sem libmpv no sistema: componente mostra erro legível, app não cai.
- Exemplo `video_player` + teste que abre um vídeo curto gerado e confere pixel.
- Otimização futura (fora deste escopo): textura compartilhada D3D11 via `WGL_NV_DX_interop`.

## Abordagens consideradas (vídeo)

| | Como | Troca |
|---|---|---|
| **A (escolhida)** | render API → textura | Cópia GPU→CPU→GPU (~500 MB/s em 1080p60). Componente normal, todos os backends. |
| B | janela filha + controles em janela por cima | Mais leve, mas 2 janelas p/ sincronizar, briga com decorations. |
| C | DirectComposition nos backends | Reescrever 5 backends; OpenGL/Vulkan sem DComp nativo. |

## Decisões deixadas ao construtor

- Nomes exatos de API, tamanho da borda de redimensionar, constantes de spring.
- Como transformar o hit-test (matriz inversa simples, só translate+scale).
- Origem da libmpv (procurar ao lado do exe, depois PATH — igual ao desktop).
