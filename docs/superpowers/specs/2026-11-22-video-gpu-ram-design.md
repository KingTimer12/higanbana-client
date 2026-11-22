# Vídeo na GPU, RAM ao navegar e tela preta no próximo episódio — design

## Por quê
O foco é CPU enquanto se assiste anime. Hoje o player gasta CPU levando cada quadro da GPU para a RAM e de volta. Depois vem a RAM: ela começa em ~50 MB e cresce conforme se navega pelo app, o que pode atrapalhar quem está vendo anime. E um bug: ao pular para o próximo episódio pelo player, o vídeo fica preto até sair e entrar de novo.

## Como é hoje
- `hwdec=auto-copy`: o quadro decodificado na GPU volta para a RAM.
- Render API "sw" do mpv (`video/render.rs`): converte e escala na CPU até 1440p, aloca um `Vec` novo por quadro e roda um loop que fixa o alfa em 255.
- `Player::frame` entrega o quadro como `ImageData::dynamic`, e o `ImageCache` sobe os pixels para a GPU (`hg_image_update`).
- Tela preta: `load_at` zera `first_frame`, que só volta a `true` quando o `vo-configured` muda. Na troca de arquivo ele não muda, e `video/view/mod.rs:130` espera esse sinal para sempre.

## Objetivos
1. O próximo episódio mostra imagem sem precisar sair e entrar.
2. **C** — o caminho pela CPU fica mais leve: o quadro é desenhado no tamanho em que aparece na tela (com teto de 1440p), o buffer é reaproveitado e o loop do alfa sai.
3. **A** — no D3D11 (backend padrão), o mpv desenha com OpenGL direto numa textura D3D11 compartilhada (`WGL_NV_DX_interop2`), com hwdec na GPU e sem passar pela CPU. O caminho C fica como reserva automática (outro backend, sem a extensão ou com falha).
4. RAM: medir tela por tela, achar o que cresce sem ser liberado e corrigir.

## Fora do escopo
- Janela filha do mpv (caminho B).
- Vídeo na GPU para os backends D3D12, Vulkan, OpenGL e wgpu: eles usam o caminho C.
- Mudar o visual do player.

## Restrições
- Medir CPU (1 núcleo) e RAM (working set) tocando o mesmo episódio antes e depois de cada passo.
- Arquivos pequenos, cada um com uma responsabilidade. TDD. Sem push. Sem trailer de atribuição nos commits.
- Nunca varrer a nuvem Anitsu em script: só abrir as pastas que o teste precisa.
- C do núcleo sem headers C++: vtables à mão, com os slots conferidos no SDK (como em `text/font_files.c`).

## Abordagens consideradas
- **A — textura D3D11 compartilhada (escolhida).** CPU perto de zero, controles continuam desenhados pela lib. Risco médio-alto: ponte entre OpenGL e D3D11 e sincronização entre threads.
- **B — janela filha do mpv.** É o mais leve, mas exige uma segunda janela transparente para os controles. Frágil. Descartada.
- **C — CPU mais leve (escolhida, antes do A).** Baixo risco e ganho parcial; vira a reserva do A.

## Decisões de como fazer
- **Sinal de primeiro quadro.** `first_frame` passa a vir da thread de render: o primeiro quadro entregue depois de um `load`/`load_at` (contador de geração).
- **Quadro opaco.** O loop que fixa o alfa sai. No lugar, a imagem ganha a opção "opaca" (`hg_image_create` com flag), e o shader ignora o alfa.
- **Tamanho alvo.** O view informa o tamanho da área do vídeo em pixels físicos (`Player::set_view_size`). O render usa o menor entre esse tamanho e o tamanho do vídeo, com teto de 1440p.
- **Buffers.** Dois `Vec` alternados entre a thread de render e a UI: a UI devolve o buffer depois do upload.
- **Arquitetura do A.**
  - A thread de render tem um contexto OpenGL escondido e um device D3D11 próprio.
  - Duas texturas D3D11 compartilhadas (`SHARED_KEYEDMUTEX`) servem de alvo do mpv via `wglDXRegisterObjectNV`.
  - A UI abre as texturas no device dela (`OpenSharedResource1`) e as desenha como `hg_image` externa. Assim o contexto imediato da UI nunca é tocado por outra thread.
  - Se o spike mostrar que um device só basta (interop no device da UI com lock), isso vira ruling com o motivo.
- **hwdec no A.** `auto`, com reserva para `auto-copy` se a interop recusar.

## O que fica para quem executa
- A forma exata da API C da imagem externa e os detalhes de sincronização: decididos no spike (Tarefa 3), com ruling.
- O que corrigir na RAM: decidido pelas medições (Tarefa 5).
