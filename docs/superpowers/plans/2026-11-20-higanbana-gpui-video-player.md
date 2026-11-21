# higanbana-gpui Player de Vídeo Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Componente `VideoPlayer` (shadcn-style) no higanbana-gpui que toca vídeo via libmpv como imagem atualizada por quadro, em qualquer backend.

**Architecture:** libmpv carregada em runtime (loader baseado em `higanbana-desktop/src/video/mpv.rs`). Render API **sw** do mpv numa thread própria escreve o quadro (RGB0) num buffer; o quadro pronto vai para um slot compartilhado e acorda a UI (`hg_window_wake`). No render, o `Player` passa o quadro novo para uma `ImageData` dinâmica; o `ImageCache` chama `hg_image_update` (textura atualizada sem recriar quando o backend suporta). Estado (tempo, duração, pausa, volume, faixas) vem de `observe_property` numa thread de eventos.

**Tech Stack:** C11 core, Rust, libmpv 2.x (`libmpv-2.dll`), `libloading`.

**Spec:** `docs/superpowers/specs/2026-11-20-higanbana-gpui-anim-decor-video-design.md` (seção 3)

## Global Constraints

- Branch `feat/higanbana-gpui`; commits sem trailer.
- Feature cargo `video` no `higanbana-gpui` (desligada por padrão); sem libmpv no sistema → erro legível, nunca pânico.
- Procura da DLL: env `HIGANBANA_MPV_DLL`, ao lado do exe, `<exe>/lib`, `crates/higanbana-desktop/lib` (dev).
- CPU ociosa: player pausado/sem vídeo não gera frames (só eventos do mpv acordam a UI).
- Sem arquivo deus: `src/video/{mod,api,player,render,state}.rs`, componente em `src/components/video/`.

## Ruling (registrado antes de começar)

- Spec dizia contexto WGL oculto + FBO + 2 PBOs. Trocado pela render API `sw` do mpv: mesmo contrato (buffer RGBA na CPU → `hg_image_update`), sem GL. Custo se errado: CPU maior em 4K; caminho GL pode ser plugado atrás de `video::render` depois.
- `texture_update` opcional na vtable: rápido em D3D11/OpenGL/WGPU; D3D12/Vulkan caem no fallback (destrói e recria a textura no próximo draw). Os 5 funcionam.

## Review Focus

- Fechar o player/janela com a thread de render no meio de um quadro: sem crash nem deadlock (teste Task 3 `drop_while_playing`).
- Vídeo que muda de resolução no meio (nova faixa / arquivo): imagem recriada no tamanho novo (teste Task 2 `dynamic_image_resize_recreates`).
- Troca de backend com vídeo tocando: textura recriada no backend novo (pixels guardados no core) — teste Task 1 `update_survives_backend_switch`.
- Arquivo inválido: estado `Error(msg)` e UI mostra mensagem (teste Task 3 `invalid_file_reports_error`, Task 4 `error_overlay`).
- Controles somem sem mexer o mouse e não deixam timer rodando quando já sumiram (teste Task 4 `controls_autohide_then_idle`).

---

### Task 1: Core — `hg_image_update` e `hg_window_wake`

**Files:** Modify `core/include/hgui.h`, `core/src/image/image.c`, `core/src/backend.h` (`texture_update` opcional), backends D3D11/OpenGL (`core/src/backends/...`), `higanbana-gpui-sys/src/wgpu_backend/{ffi,mod,images}.rs`, `core/src/window/window_win32.c`. Test: `higanbana-gpui-sys/tests/image_update.rs`, `tests/wake.rs`.

**Interfaces:**
```c
/* Troca os pixels (mesmo tamanho). Textura atualizada no lugar se o backend suporta. */
hg_result hg_image_update(hg_image* img, const uint8_t* rgba, uint32_t stride);
/* Thread-safe: acorda wait_event com HG_EVENT_WAKE. */
void      hg_window_wake(hg_window* win);
/* vtbl: hg_result (*texture_update)(hg_context*, struct hg_image*); NULL = recria */
```
Pixels de entrada: RGBA8 reto, como `hg_image_create` (o core pré-multiplica igual).

- [ ] Testes: `update_changes_drawn_pixels` (para cada backend disponível: cria 4x4 vermelho, desenha, lê pixel; update para azul; desenha; pixel azul); `update_wrong_args` (stride < w*4 → HG_ERR_INVALID); `update_survives_backend_switch` (update, troca D3D11→OpenGL, pixel azul); `wake_from_other_thread` (thread chama `hg_window_wake`; `hg_window_wait_event` retorna `HG_EVENT_WAKE`).
- [ ] FAIL → implementar → `cargo test -p higanbana-gpui-sys` PASS → commit `feat(gpui): hg_image_update e hg_window_wake no core`.

### Task 2: Rust — imagem dinâmica, Waker, Event::Wake

**Files:** Modify `src/element/image_data.rs`, `src/paint/images.rs`, `src/gfx/image.rs` (`update`), `src/gfx/window.rs` (`waker`), `src/gfx/event.rs`, `src/app/{run,events}.rs`, `src/cx/render.rs`. Create `src/gfx/waker.rs`.

**Interfaces:**
- `ImageData::dynamic(width, height) -> ImageSource`; `ImageData::set_frame(&self, width, height, pixels: Vec<u8>)` (troca pixels; tamanho pode mudar; incrementa `version`).
- `ImageCache::prepare`: id existente com pixels pendentes → mesmo tamanho: `Image::update`; tamanho novo: recria.
- `Image::update(&mut self, pixels: &[u8]) -> Result<()>`.
- `pub struct Waker` (`Clone + Send + Sync`), `Waker::wake(&self)`; vira no-op depois que a janela morre. `Window::waker()`; `RenderCx::waker() -> Waker`.
- `Event::Wake` → Runner marca `Rebuild`.

- [ ] Testes: `dynamic_image_update_in_place` (cache com contexto real D3D11: prepare, set_frame mesmo tamanho → mesmo `Image` atualizado, len 1); `dynamic_image_resize_recreates`; `waker_noop_after_window_drop`; `wake_event_rebuilds` (Runner/handle unit se possível, senão via `Event::from_raw`).
- [ ] FAIL → implementar → `cargo test -p higanbana-gpui` PASS → commit `feat(gpui): imagem dinâmica e waker entre threads`.

### Task 3: `video` — loader libmpv e `Player`

**Files:** Create `src/video/mod.rs`, `api.rs` (símbolos + procura DLL), `player.rs` (handle, comandos), `render.rs` (thread sw render + slot), `state.rs` (`PlayerState` observado). Modify `Cargo.toml` (feature `video`, dep `libloading`). Test: `tests/video.rs` (`#![cfg(feature = "video")]`).

**Interfaces:**
- `pub struct Player`; `Player::new(waker: Waker) -> Result<Player, String>`; `load(&self, path_or_url: &str)`; `play()`, `pause()`, `toggle_pause()`, `seek(secs: f64)`, `seek_relative(secs)`, `set_volume(0..=100)`, `toggle_mute()`, `set_speed(f64)`, `set_audio_track(Option<i64>)`, `set_sub_track(Option<i64>)`.
- `Player::state(&self) -> PlayerState`: `{ status: Status::{Idle, Loading, Playing, Paused, Ended, Error(String)}, position: f64, duration: f64, volume: f64, muted: bool, speed: f64, audio_tracks: Vec<Track>, sub_tracks: Vec<Track>, audio: Option<i64>, sub: Option<i64>, video_size: Option<(u32, u32)> }`, `Track { id: i64, title: String, lang: String }`.
- `Player::frame(&mut self) -> Option<ImageSource>`: devolve a `ImageData` dinâmica atualizada se chegou quadro novo (mesma `ImageSource` sempre).
- `Player::detached() -> Player` (sem mpv; para testes de UI) e `Player::set_state_for_test`.
- Render: formato `rgb0`, tamanho = `dwidth×dheight` limitado a 2560×1440 (mantém proporção); alfa forçado 255 antes de publicar. Drop: para a thread de render (flag + notify), libera `mpv_render_context` antes de `mpv_terminate_destroy`.

- [ ] Testes (usam `av://lavfi:color=c=red:size=64x48:rate=10` e `av://lavfi:testsrc`): `plays_lavfi_and_delivers_frame` (quadro chega em ≤5 s, pixel central vermelho, `video_size == (64,48)`); `pause_and_seek_update_state`; `invalid_file_reports_error`; `drop_while_playing` (cria, toca, dropa em loop 5×, sem travar >10 s); `missing_dll_is_error` (`HIGANBANA_MPV_DLL` apontando para arquivo inexistente e sem outros candidatos → `Err` com "libmpv").
- [ ] FAIL → implementar → `cargo test -p higanbana-gpui --features video --test video` PASS → commit `feat(gpui): Player com libmpv (render sw) atrás da feature video`.

### Task 4: Componente `VideoPlayer`

**Files:** Create `src/components/video/{mod,player_view,controls,tests}.rs`. Modify `src/components/mod.rs` (`#[cfg(feature = "video")]`).

**Interfaces:**
- `VideoPlayer::new(id, lens: impl Fn(&mut V) -> &mut VideoState + 'static)`, `.fit(Fit::Contain | Fit::Cover)`, `.render(cx)`. `pub struct VideoState { pub player: Player, last_activity: Instant, last_click: Option<Instant> }`, `VideoState::new(player)`.
- Superfície: `img(frame)` centrado, contain/cover, fundo preto; sem quadro: spinner (Loading) ou mensagem (Error).
- Controles (gradiente + painel inferior): play/pausa, tempo `mm:ss / mm:ss` (h:mm:ss se ≥1 h), `Slider` de seek (0..duração), mute + `Slider` de volume, `Select` velocidade (0.5, 0.75, 1, 1.25, 1.5, 2), `Select` áudio e legenda (com "Desligada"), botão fullscreen. Ícones Segoe MDL2 (play E768, pause E769, volume E767, mute E74F, fullscreen E740, sair E73F).
- Auto-ocultar: visíveis por 2,5 s após o último movimento/tecla (ou sempre se pausado); fade com `presence`; cursor some junto (`CursorStyle::None` se existir, senão manter). `request_redraw_in` só enquanto visíveis e tocando.
- Teclas (elemento focável): Espaço/K pausa, ←/→ ±5 s, ↑/↓ volume ±5, F fullscreen, M mudo, Esc sai do fullscreen. Duplo clique (≤400 ms) = fullscreen; clique simples = pausa.

- [ ] Testes (árvore sem GPU, `Player::detached`): `controls_show_time_and_progress` (pos 65 / dur 3725 → "1:05 / 1:02:05"); `space_toggles_pause` (comando registrado no player detached); `double_click_fullscreen` (dois cliques em 200 ms → `WindowCommand::ToggleFullscreen`, sem pausar); `controls_autohide_then_idle` (3 s sem atividade tocando → controles ausentes e nenhum `wake` agendado); `error_overlay` (status Error("x") → texto "x").
- [ ] FAIL → implementar → `cargo test -p higanbana-gpui --features video` PASS → commit `feat(gpui): componente VideoPlayer`.

### Task 5: Exemplo e verificação real

**Files:** Create `examples/video_player.rs` (`required-features = ["video"]`; arg = caminho/URL, padrão `av://lavfi:testsrc2=size=1280x720:rate=30`; `Decorations::Custom` + `TitleBar` + `VideoPlayer`).

- [ ] Build release sem warnings; rodar com testsrc: screenshot com vídeo + controles; seek pelo slider; fullscreen por F; CPU pausado ≈0%; medir CPU/RAM tocando 720p30 e registrar. Testar com um arquivo local se houver (`mkv` com legenda) — registrar resultado.
- [ ] Commit `feat(gpui): exemplo video_player`.
