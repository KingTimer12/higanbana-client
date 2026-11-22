# Vídeo na GPU, RAM e próximo episódio — plano de implementação

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Próximo episódio sem tela preta, depois vídeo mais barato pela CPU (C), depois vídeo desenhado na GPU no D3D11 (A), depois RAM estável ao navegar.

**Architecture:** O mpv continua na thread `mpv-render`.
- C: deixa o render "sw" mais leve (tamanho na tela, buffers reaproveitados, imagem opaca).
- A: troca o alvo para OpenGL → textura D3D11 compartilhada, aberta pela UI como `hg_image` externa.
- O caminho C fica como reserva automática.

**Tech Stack:** Rust (higanbana-gpui, higanbana-desktop), C11 do núcleo (DirectX 11, WGL), libmpv 2.x.

**Spec:** `docs/superpowers/specs/2026-11-22-video-gpu-ram-design.md`

## Global Constraints
- Medir CPU (1 núcleo) e working set tocando o mesmo episódio antes e depois de cada tarefa de vídeo, e anotar no ledger.
- Arquivos pequenos, uma responsabilidade cada.
- Nunca varrer a nuvem Anitsu em script: só abrir o episódio ou pasta do teste.
- Sem push. Commits sem trailer de atribuição.
- C do núcleo sem headers C++: slots de vtable conferidos no SDK (`Windows Kits/10/Include/*/um`).
- O caminho sw tem que continuar funcionando em todos os backends.

## Review Focus
1. Trocar de episódio enquanto o anterior ainda carrega (dois `load_at` seguidos): a imagem tem que aparecer no último.
2. Redimensionar ou entrar em tela cheia durante a reprodução: o tamanho alvo muda sem quadro esticado ou preto.
3. Caminho A sem `WGL_NV_DX_interop2` (ou com o backend trocado em runtime): cai no sw sem tela preta.
4. Fechar o player no meio de um quadro (A): sem travar e sem vazar textura (`gpu_bytes` volta).
5. Vídeo só de áudio: nada de spin de CPU na thread de render.

---

### Task 1: Primeiro quadro depois de trocar de arquivo

**Files:**
- Modify: `crates/higanbana-gpui/src/video/render.rs`, `video/player.rs`, `video/state.rs`
- Test: `crates/higanbana-gpui/tests/video.rs`

**Interfaces:**
- Produces: `PlayerState.first_frame` = um quadro já foi entregue depois do último `load`/`load_at`. O `vo-configured` deixa de alimentar esse campo.

- [ ] **Step 1: Teste RED** `troca_de_arquivo_volta_a_ter_quadro`: player real.
  - `load_at("av://lavfi:color=c=red:s=320x240:d=30", None)`, esperar `first_frame`.
  - `load_at("av://lavfi:color=c=blue:s=320x240:d=30", None)`.
  - Esperar até 5 s por `first_frame == true` e `last_pixel(10,10)` azul.
  - Repetir com dois `load_at` seguidos (Review Focus 1).
- [ ] **Step 2:** Rodar `cargo test -p higanbana-gpui --features video --test video troca_de_arquivo` → FAIL (`first_frame` fica `false`).
- [ ] **Step 3:** Implementar.
  - Contador de geração (`AtomicU64`) incrementado em `load`/`load_at`.
  - A thread de render marca `first_frame = true` ao entregar um quadro se a geração vista mudou.
  - `load` também zera `first_frame`.
- [ ] **Step 4:** Rodar o teste → PASS. Rodar a suíte do crate.
- [ ] **Step 5:** No app real, pular para o próximo episódio pelo player e tirar foto: imagem visível.
- [ ] **Step 6:** Commit `fix(video): primeiro quadro vem do render; próximo episódio não fica preto`.

### Task 2: C — sw mais leve

**Files:**
- Modify: `video/render.rs` (tamanho e buffers), `video/player.rs` (`set_view_size`, devolver buffer)
- Modify: `element/image_data.rs` + `paint/images.rs` (imagem opaca); núcleo `image/*` + shaders de imagem dos backends (flag opaca)
- Modify: `crates/higanbana-desktop/src/video/view/*.rs` (informar o tamanho da área)
- Test: `video/render.rs` (unit), `tests/video.rs`, `tests/gpu.rs`

**Interfaces:**
- Produces:
  - `Player::set_view_size(&self, w: u32, h: u32)` (pixels físicos).
  - `render::target_size(video: (u32,u32), view: Option<(u32,u32)>) -> (u32,u32)`.
  - `ImageData::dynamic_opaque(w, h)`.
  - `hg_image_create` passa a aceitar `HG_IMAGE_OPAQUE` (o shader usa alfa 1).

- [ ] **Step 0:** Medir a linha de base. Release, episódio 1080p tocando 20 s em janela 1296×839: CPU e working set no ledger.
- [ ] **Step 1: Testes RED.**
  - `target_size_usa_a_vista`: vídeo (1920,1080) e vista (1280,720) → (1280,720). Vista maior que o vídeo → tamanho do vídeo. Vista `None` → comportamento atual. Proporção do vídeo mantida dentro da vista.
  - `imagem_opaca_ignora_alfa` em `tests/gpu.rs`: imagem 2×2 com alfa 0 e flag opaca, desenhada sobre fundo verde → pixel com a cor da imagem.
  - `buffer_volta_para_o_render`: depois de `frame()` e do upload, o próximo quadro reaproveita a capacidade (contador de alocações exposto só em teste).
- [ ] **Step 2:** Rodar → FAIL.
- [ ] **Step 3:** Implementar.
  - Tamanho: `target_size` com a vista.
  - Opaco: flag no núcleo e nos shaders dos 5 backends; tirar o loop do alfa.
  - Buffers: o slot carrega `Vec` reaproveitado; a UI devolve o buffer depois de `take_pixels`.
  - Desktop: chamar `set_view_size` no render do player quando a área mudar.
- [ ] **Step 4:** Rodar os testes e a suíte → PASS.
- [ ] **Step 5:** Medir do mesmo jeito do Step 0. Foto do player normal e em tela cheia (Review Focus 2). Anotar no ledger.
- [ ] **Step 6:** Commit `perf(video): sw no tamanho da tela, buffers reaproveitados, imagem opaca`.

### Task 3: A — spike da ponte OpenGL → D3D11

**Files:**
- Create: `crates/higanbana-gpui/core/src/video_gl/` (contexto WGL escondido, extensões `WGL_NV_DX_interop2`, texturas compartilhadas): `gl_hidden.c`, `dx_interop.c`, `shared_tex.c` mais o header.
- Modify: `core/include/hgui.h`; `backends/d3d11/d3d11_images.c` (abrir textura compartilhada como `hg_image`)
- Test: `crates/higanbana-gpui-sys/tests/video_gl.rs`

**Interfaces:**
- Produces (a forma final é decidida aqui, com ruling):
  - `hg_video_target_create(hg_context*, uint32_t w, uint32_t h, hg_video_target** out)`: device D3D11 próprio, 2 texturas `SHARED_KEYEDMUTEX`, contexto GL.
  - `hg_video_target_begin(t, uint32_t* gl_fbo)` / `hg_video_target_end(t)`: na thread de render.
  - `hg_video_target_image(t, hg_image** out)`: na UI, a última textura pronta como `hg_image` externa.
  - `hg_video_target_destroy`.

- [ ] **Step 1: Teste RED** `gl_limpa_vermelho_e_d3d11_ve`.
  - Criar o alvo 64×64. Na thread de teste: `begin`, `glClear` vermelho no FBO, `end`.
  - Na UI: desenhar a `hg_image` numa janela D3D11 e ler o pixel → vermelho.
  - Se a máquina não tiver a extensão, o teste reporta `skip` com o motivo.
- [ ] **Step 2:** Rodar → FAIL (símbolos inexistentes).
- [ ] **Step 3:** Implementar o mínimo para passar.
  - Se um device só bastar, ou se o keyed mutex não funcionar com a interop: ledger `Ruling:` com o que foi feito e o motivo.
- [ ] **Step 4:** Rodar → PASS. Teste `destroy_libera_gpu_bytes` (Review Focus 4).
- [ ] **Step 5:** Commit `feat(gpui-core): alvo de vídeo OpenGL em textura D3D11 compartilhada`.

### Task 4: A — mpv na GPU

**Files:**
- Create: `crates/higanbana-gpui/src/video/render_gl.rs` (render API "opengl" com `get_proc_address` do contexto escondido)
- Modify: `video/render.rs` (escolha do caminho), `video/player.rs`, `video/options.rs` (hwdec do caminho GPU)
- Modify: `paint/images.rs` (`ImageSource` externa vinda do alvo)
- Test: `tests/video.rs`

**Interfaces:**
- Consumes: Task 3 (`hg_video_target_*`), Task 1 (geração e primeiro quadro), Task 2 (`target_size`).
- Produces: `Player::render_path() -> RenderPath { Gpu, Cpu }` (diagnóstico e testes). O `Player::frame()` continua devolvendo `ImageSource`.

- [ ] **Step 1: Testes RED.**
  - `gpu_mostra_lavfi_vermelho`: backend D3D11, `load_at(lavfi red)`, frame desenhado na janela → pixel vermelho, `render_path() == Gpu`.
  - `sem_interop_cai_no_sw`: forçar a falha com a env `HIGANBANA_VIDEO_GPU=0` → `Cpu` e imagem presente (Review Focus 3).
  - Repetir o teste da Task 1 no caminho GPU.
- [ ] **Step 2:** Rodar → FAIL.
- [ ] **Step 3:** Implementar.
  - `render_gl` cria o contexto de render "opengl" e renderiza no FBO do alvo.
  - `hwdec=auto` no caminho GPU, com `auto-copy` se a interop recusar.
  - Troca de backend em runtime: o player volta ao sw.
- [ ] **Step 4:** Rodar os testes e a suíte → PASS.
- [ ] **Step 5:** Medir como na Task 2 (mesmo episódio). Foto normal e em tela cheia. Conferir legendas e controles por cima. Ledger.
- [ ] **Step 6:** Commit `feat(video): mpv desenha na GPU (D3D11) com reserva no sw`.

### Task 5: RAM ao navegar

**Files:** decididos pela medição; cada correção num arquivo da responsabilidade certa.

- [ ] **Step 1:** Script de medição `scratchpad/ram_walk.ps1`.
  - Abre o app e percorre: Início → Animes → um título → episódios → player (20 s) → volta → Mangás → leitor → volta → OSTs → álbum → Início.
  - Repete o percurso 3 vezes e anota o working set e o `gpu_bytes`/`atlas` (via `show_stats` ou log) depois de cada tela.
  - Só abre as pastas do percurso.
- [ ] **Step 2:** Rodar e montar a tabela. O que cresce de volta em volta é vazamento; o que estabiliza é cache. Ledger.
- [ ] **Step 3:** Para cada vazamento: teste RED que reproduz (ex.: o cache de imagens fica limitado a N depois de visitar M telas), corrigir, teste GREEN.
  - Para caches sem teto: definir o teto (ruling com o número e o motivo).
- [ ] **Step 4:** Rodar o script de novo: o working set da 3ª volta fica até 10% acima da 1ª. Ledger.
- [ ] **Step 5:** Commit por correção: `perf(desktop): <o que> libera ao sair da tela`.
