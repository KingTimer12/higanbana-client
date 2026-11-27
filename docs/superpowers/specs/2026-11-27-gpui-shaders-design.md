# higanbana-gpui — shaders do usuário + README da crate

## Intenção

Quem usa a `higanbana-gpui` hoje só desenha quads, imagens, texto, ícones e polígonos com shaders
fixos. O usuário quer uma função para **escrever o próprio shader** e com ela montar componentes
como um **LiquidGlass** (vidro que refrata e desfoca o que está atrás). Por fim, um **README** da
crate documentando todas as funções, usos e componentes.

## Objetivos

- Shader do usuário em **WGSL único**, funcionando nos **5 backends** (D3D11, D3D12, OpenGL, Vulkan, WGPU).
- Uso como **fundo de uma `div`** (filhos desenham por cima) e direto no canvas.
- Acesso ao **backdrop** (o que já foi desenhado atrás), para vidro/blur/refração.
- `time` e até 16 `vec4` de parâmetros por desenho; animação contínua opcional.
- Erro de compilação legível (linha/coluna **do código do usuário**).
- Componente de exemplo `LiquidGlass` na galeria, feito só com a API pública.
- `crates/higanbana-gpui/README.md` completo.

## Não-objetivos

- Vertex shader do usuário, bindings próprios, texturas extras do usuário, múltiplas passadas,
  compute. (Abordagem 2, descartada.)
- Hot-reload de arquivo `.wgsl`.
- Shader no caminho de vídeo.

## Anti-objetivos

- Custo de CPU por quadro quando não há shader na tela (zero mudança no caminho atual).
- Shader animado redesenhando fora da tela ou com a janela parada.
- Arquivo deus: cada backend ganha arquivo próprio para shaders (`*_shaders_user.c` ou similar).

## Abordagens consideradas

1. **Função `shade` com prelúdio da lib — escolhida.** O usuário escreve só
   `fn shade(p: ShadeInput) -> vec4<f32>`; a lib monta o resto (vértice, cantos, clip, blend).
2. Pipeline livre (vertex + fragment + bindings). Rejeitada: exige conhecer o layout de cada backend
   e trava a evolução interna.

Linguagem (decisão do usuário): **A — WGSL único** traduzido em tempo de execução pelo `naga`
(já usado no build, `=30.0.1`) para HLSL, GLSL e SPIR-V.

## Design

### Contrato WGSL

A lib concatena: **prelúdio** + código do usuário + **pontos de entrada**. Prelúdio expõe:

```wgsl
struct ShadeInput {
    uv: vec2<f32>,        // 0..1 dentro do retângulo
    px: vec2<f32>,        // posição em pixels dentro do retângulo
    size: vec2<f32>,      // tamanho do retângulo em pixels
    screen: vec2<f32>,    // posição em pixels na janela
    time: f32,            // segundos desde a criação do Shader
    scale: f32,           // DPI
    radius: vec4<f32>,    // cantos tl,tr,br,bl em pixels
}
fn param(i: u32) -> vec4<f32>            // 16 vec4 do app
fn backdrop(screen_px: vec2<f32>) -> vec4<f32>      // amostra bilinear do que está atrás
fn backdrop_blur(screen_px: vec2<f32>, radius: f32) -> vec4<f32>  // helper (amostras fixas)
fn rounded_sdf(px: vec2<f32>, size: vec2<f32>, radius: vec4<f32>) -> f32
```

A cor devolvida é **não pré-multiplicada**; a lib aplica cantos arredondados (anti-alias pela SDF),
clip e blend pré-multiplicado como nos quads. `backdrop*` sem `.backdrop(true)` devolve
transparente.

Erros: o `naga` valida o módulo completo; a lib desconta as linhas do prelúdio e devolve
`ShaderError { line, column, message }` apontando para o código do usuário.

### API Rust

```rust
let glass = Shader::wgsl(include_str!("glass.wgsl"))?   // Result<Shader, ShaderError>
    .backdrop(true)        // copia a região de trás antes de desenhar
    .animated(true);       // pede quadro contínuo enquanto visível
div().size(300., 200.).rounded(24.)
    .shader(&glass, params![tint, [refr, blur, 0., 0.]])
    .child(text("Olá"))
canvas.shader(rect, radius, &glass, &params);
```

- `Shader` é barato de clonar (`Rc`/`Arc` do fonte + traduções em cache por backend).
- Tradução `naga` + compilação nativa acontecem **uma vez por (shader, backend)**, na primeira vez
  que aparece na tela; troca de backend em tempo de execução (`context_switch.c`) recompila sob
  demanda, como o cache de imagens faz.
- Shader inválido usado sem checar → retângulo magenta + `eprintln!` uma vez.
- `animated(true)`: o elemento pede o próximo quadro via `Animator` só se foi pintado neste quadro
  (fora da tela/janela minimizada não anima).
- Feature Cargo `shaders` (padrão ligada) para quem não quer o `naga` no binário.

### Núcleo C

- `hgui.h`: `hg_shader` opaco; `hg_shader_create(ctx, const hg_shader_code* code, hg_shader** out,
  char* err, size_t err_len)` onde `hg_shader_code` tem o código já traduzido para o backend ativo
  (HLSL texto / GLSL texto / SPIR-V palavras); `hg_shader_destroy`; `hg_draw_shader(dl, hg_shader*,
  const hg_shader_draw*)` com `rect`, `radius[4]`, `time`, `scale`, `params[16][4]`, `flags`
  (`HG_SHADER_BACKDROP`).
- Draw list: desenho de shader **fecha a camada atual**, vira um lote próprio e abre camada nova
  (respeita "o que veio antes fica atrás, o que vem depois fica por cima"; vale com a ordem fixa
  quads→imagens→texto de cada camada).
- Backdrop: por superfície, uma textura do tamanho da janela (criada na 1ª vez, recriada no resize);
  antes de cada lote com `BACKDROP`, copia **só o retângulo** (com margem para blur, recortado à
  janela) do alvo atual para ela.
  - D3D11: `CopySubresourceRegion`; HLSL compilado com `D3DCompile` (`d3dcompiler_47.dll` carregada
    dinamicamente, padrão dos loaders).
  - D3D12: `CopyTextureRegion` com barreiras; mesmo HLSL/DXBC.
  - OpenGL: `glCopyTexSubImage2D` (cuidar do eixo Y invertido); GLSL do naga compilado no driver.
  - Vulkan: fim do render pass → `vkCmdCopyImage` → retoma; SPIR-V direto.
- Um uniform/constant buffer por desenho de shader (ou anel por quadro).

### LiquidGlass (exemplo)

`examples/gallery` ganha seção com componente `liquid_glass(...)` em arquivo próprio: refração na
borda (deslocamento do backdrop pela normal da SDF), blur, leve tinta e brilho especular animado.
Exemplo também em `examples/elements`.

### Medição

- CPU/RAM de app sem shader: igual a antes (mesma galeria, mesmo backend).
- Com 1 e 5 vidros na tela: CPU por quadro, VRAM extra (textura de backdrop), tempo de compilação
  do 1º uso, tamanho do binário com/sem feature `shaders`.

### Testes

- Rust: tradução e mapeamento de linha de erro; WGSL inválido → `ShaderError` com linha certa.
- `higanbana-gpui-sys/tests`: por backend disponível, desenha quad vermelho + shader que devolve
  `backdrop(...)` invertido → lê pixel (`hg__window_read_pixel`) e confere; shader sem backdrop
  devolve cor sólida; cantos arredondados recortam; ordem (shader acima do que veio antes, abaixo do
  que vem depois).

## Decisões do autor do design (corrigíveis)

- **Backend WGPU (5º, em Rust, `higanbana-gpui-sys/src/wgpu_backend/`, último fallback do AUTO)
  também recebe suporte completo**: usa o WGSL direto, sem tradução; backdrop via
  `copy_texture_to_texture`. "Todos os backends" = 5.
- **`.shader()` substitui só o preenchimento** (cor/gradiente) da div. Sombra desenha antes (entra no
  backdrop, como sombra real de vidro); borda desenha por cima do shader.
- **Opacidade herdada vale**: `hg_shader_draw` ganha `opacity`, multiplicada no alpha final.
  Escala/transformação: o `rect` passado já é o retângulo final em pixels, então `size`/`px` seguem
  a escala sem campo extra.
- **"Janela parada"** = minimizada/oculta ou elemento não pintado no quadro. Janela sem foco ou
  coberta continua animando (como o vídeo).
- **Erro do compilador nativo** (`D3DCompile`/driver GL/pipeline Vulkan/WGPU), que só aparece no 1º
  uso ou após troca de backend: retângulo magenta + `eprintln!` uma vez + `Shader::last_error() ->
  Option<ShaderError>` para o app consultar.
- **Feature `shaders`** em `higanbana-gpui` (padrão ligada; repassa para `-sys`, que liga `naga`
  em tempo de execução com `wgsl-in`, `hlsl-out`, `glsl-out`, `spv-out`). Desligada: a API continua
  existindo e `Shader::wgsl` devolve `Err(ShaderError::Unsupported)` — sem `cfg` no código do app.

## README da crate

`crates/higanbana-gpui/README.md`, escrito a partir do código real: visão geral e arquitetura
(Rust + núcleo C + backends), app e laço (`App`, `View`, `RenderCx`, `EventCx`, timers, tarefas,
mailbox), elementos e estilo, layout, eventos/foco/teclado, texto e fontes, imagens e ícones,
animação, temas, **todos os componentes** (por pasta de `components/`), overlays, vídeo,
backends e troca, shaders, exemplos (`cargo run --example ...`), testes.

## Deixado ao construtor

- Layout exato do uniform buffer e anel por quadro.
- Quantidade de amostras do `backdrop_blur`.
- Nome dos arquivos C por backend, mantendo um por responsabilidade.
