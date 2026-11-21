# Migração do higanbana-desktop para higanbana-gpui

## Intenção

Largar o gpui-kit de vez. O `higanbana-desktop` passa a usar só a nossa lib `higanbana-gpui`. Todo componente que o desktop usa e a lib não tem entra **na lib** (reutilizável, com testes e seção na galeria), nunca como gambiarra no app.

O player de vídeo é prioridade de fidelidade: tem que ficar **visualmente idêntico** ao atual (`crates/higanbana-desktop/src/video/view.rs`). As outras telas também devem manter o mesmo visual (cores, espaçamentos, tipografia, motion).

## Objetivos

- `higanbana-desktop` compila sem `gpui`/`gpui-kit` no `Cargo.toml`.
- Todas as telas atuais funcionando: login, home (hero carousel, continuar, descobrir), catálogo + filtros, título, episódios, pasta, franquia, estante, leitor de mangá, música (álbum + barra), player de vídeo, configurações, busca (paleta), notificações, title bar + sidebar.
- Player idêntico ao atual em layout, cores, tamanhos, ícones, animações e comportamento (atalhos, auto-hide de controles **e cursor**, pular abertura, próximo episódio com contagem, miniatura no hover da barra, marcas de capítulo, menus de faixas/config/episódios, salvar progresso a cada 5s).
- Lib ganha os componentes que faltam, cada um com testes e entrada na galeria.

## Não-objetivos

- Redesenhar qualquer tela.
- Mudar lógica de negócio (API Anitsu, cache, progresso, downloads): só a camada de UI muda.
- Otimizar o pipeline de vídeo agora (buffer reuse, GPU zero-copy): fica para depois.

## Anti-objetivos (falha mesmo se "funcionar")

- Player com visual diferente do atual.
- Arquivo deus: `video/view.rs` (1.5k linhas) e `reader/view.rs` (1.3k) devem sair quebrados em arquivos por responsabilidade.
- Componente específico do app enfiado na lib, ou componente genérico escrito dentro do app.
- Varrer pastas da nuvem Anitsu em scripts de teste (rate limit, conta já foi bloqueada).
- CPU ociosa acima de ~0% com o app parado.

## Restrições

- Branch `feat/desktop-gpui` (a partir da master). Branch fica quebrada até o fim — aceito pelo usuário.
- Commits sem trailer de atribuição.
- Arquivos pequenos por responsabilidade; doc/comentários em português.
- Windows é o alvo; lib roda nos 5 backends.

## Abordagens consideradas

1. **Trocar tudo no próprio `higanbana-desktop`, de uma vez** — **escolhida pelo usuário**. Um só app, sem duplicação; branch quebrada até o fim.
2. Crate novo em paralelo e troca no final — permite merges parciais e comparação lado a lado; usuário recusou (quer a troca direta).
3. Migrar tela a tela dentro do app — inviável: dois frameworks não dividem janela/loop.

Vídeo:
1. **Vídeo desenhado pela lib (mpv render API sw → imagem dinâmica)** — **escolhida**. Funciona nos 5 backends, overlays triviais; mais CPU em 1080p+/4K (otimização posterior).
2. Janela nativa do mpv por baixo com UI transparente — CPU ~0, mas exige transparência sobre janela filha; recusada por ora.

## Decisões do how

### Fase 1 — lib: o que falta

Ordem por dependência:

1. **Ícones SVG.** Rasterizar SVG (Lucide, os mesmos ~60 arquivos de `higanbana-desktop/icons` + os do kit que o app usa) para imagem no tamanho/escala pedidos, com cor (tint) aplicada; cache por (ícone, tamanho px, escala). `icon(IconName)`/`Icon` com `.size()`/`.color()`. Botão ganha `.icon()`. Rasterizador: `resvg` (minha escolha; puro Rust).
2. **Assets.** Fonte de assets embutidos (`include_bytes!`) registrável pelo app, como o `icon_assets!` atual.
3. **Gradiente linear** em `bg` (scrims do player: preto 0.75→0 para baixo, 0.85→0 para cima) — confirmar se a lib já tem; se não, adicionar no core.
4. **Popover** ancorado (BottomRight etc.), fecha no clique fora/Esc.
5. **RadioGroup/Radio** (vertical e horizontal).
6. **Progress/ProgressBar** (tamanhos incluindo xsmall) e **Spinner** (com `animate_loop`, sem rotação se a lib não tiver; se o visual atual depender de rotação, adicionar rotação ao transform).
7. **Sidebar** (header, footer, groups, menu items, toggle/colapsar).
8. **Carousel** (conteúdo, itens, anterior/próximo, auto-avanço).
9. **Command** (paleta de busca: input + grupos + itens + navegação por teclado).
10. **Breadcrumb**, **PopupMenu**, **Kbd**, **Link**.
11. **Lista virtual** (só renderiza itens visíveis) + scroll handle programático.
12. **Cursor oculto** no core (`hg_window_set_cursor(HIDDEN)`), para o player esconder o cursor com os controles.
13. **Tema por JSON**: carregar `themes/higanbana.json` no tema da lib (mesmos tokens: primary, popover, border, radius, radius_lg, list_active, success, etc.).

### Fase 2 — player como peças

O `VideoPlayer` atual vira peças montáveis: superfície (imagem + estados), seekbar (track, buffer, played, capítulos, knob, preview por cima), botões, e lógica (`Player`, atalhos, auto-hide). O desktop monta o layout exato do `view.rs` atual com essas peças. `Player` ganha o que o desktop usa do `mpv.rs` (cache/buffer à frente, capítulos, opções de threads/cache, idioma preferido, screenshots para miniaturas — ou o `thumbs.rs` atual reaproveitado).

### Fase 3 — app

Shell (main, app, chrome: title bar, sidebar, busca, notificações) → telas na ordem: login, home, catálogo, título/episódios, pasta/franquia/estante, player, leitor, música, configurações. Remover `gpui`/`gpui-kit` do Cargo e código morto (`surface.rs` nativo).

### Verificação

- Testes de cada componente novo na lib; `cargo test` do workspace verde.
- Player: screenshot antes (app atual, na master) e depois, mesmas situações (controles visíveis, menu de faixas, settings, episódios, próximo episódio, hover na barra) e comparar.
- CPU ociosa ~0% com app parado; player 1080p medido e reportado.

## Decisões do construtor (revisáveis)

- **"Idêntico"** = igual a olho nu lado a lado (layout, cores, tamanhos, ícones, motion). Diferenças de antialiasing/rasterização são aceitas. Sem diff de pixel.
- **CPU**: player pausado = 0% (a imagem só reenvia quando chega quadro novo). 1080p tocando é medido e reportado, não bloqueia o merge.
- **Backends**: recursos novos ficam no caminho comum do core (imagens/paint), então valem para todos; validação manual só no Windows/D3D11, demais backends = compilar + testes existentes.
- **Windows nativo do desktop** (`windows-sys`): title bar/decorations já são da lib (`Decorations::Custom` + `TitleBar`). O que for específico do app (memória do processo etc.) fica no app.
- **mpv**: genérico vai pro `Player` da lib (capítulos, cache à frente, opções de threads/cache, idioma preferido de áudio/legenda). Lógica do app fica no app: miniaturas (`thumbs.rs` reaproveitado), pular abertura, contagem do próximo episódio, salvar progresso.
- **Motion**: `motion.rs` portado para `anim` da lib (já tem springs, delay e stagger via delay). Mesmas durações (160/240/420 ms) e `ease_out_cubic`.
- **Ordem**: componentes entram na lib sob demanda conforme cada tela é portada, cada um com testes; seções da galeria podem vir no fim.

## Deixado para quem constrói

- Nomes e API exata dos novos componentes (seguir o padrão da lib: builder + estado `*State` na view + lens).
- Como portar `motion.rs` (Stagger/spring) para o `anim` da lib.
- Divisão de arquivos de cada tela.
