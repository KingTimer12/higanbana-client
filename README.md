# Higanbana Client

Aplicativo desktop não oficial para navegar e baixar do acervo da [Anitsu](https://anitsu.moe/), com capa, sinopse, elenco e ficha técnica de cada título vindas do [AniList](https://anilist.co/).

> [!IMPORTANT]
> **Este projeto não tem nenhuma relação com a Anitsu.**
>
> - Usamos o acervo da Anitsu (`anitsu.moe` e `nuvem.anitsu.moe`), mas o Higanbana Client é independente: não é feito, mantido, apoiado nem endossado pela equipe da Anitsu.
> - **Todo o conteúdo que existe na plataforma é crédito da comunidade** que o reúne, organiza e mantém. Nenhum arquivo é hospedado, armazenado ou redistribuído por este projeto.
> - A finalidade do aplicativo é apenas **facilitar** o acesso ao acervo para quem já tem conta: navegar, buscar e baixar com uma interface mais confortável.
> - Os dados de capa, sinopse, elenco e equipe vêm da API pública do AniList, que também não tem relação com este projeto.

## O que faz

- **Login com Discord pelo navegador:** abre o Discord no navegador em que você já está logado; não pede senha no app.
- **Acervo inteiro:** Animes, Mangás, Doramas, Desenhos e OSTs, com busca rápida (`Ctrl K`).
- **Página de cada obra:** capa, sinopse, gêneros, nota, personagens e dubladores, equipe, obras relacionadas e ficha técnica, em abas.
- **Temporada atual na tela inicial:** os animes da temporada no AniList, com um selo nos que já estão no acervo.
- **Assistir sem baixar:** o player toca direto da nuvem, buscando só os pedaços perto de onde você está (nada vai para o disco). Toca MKV, HEVC, várias faixas de áudio e legendas `.ass` estilizadas, com:
  - botão de pular abertura;
  - contagem para o próximo episódio, que pode ser desligada;
  - continuar de onde parou;
  - legenda e áudio lembrados por release.
- **Ler mangá sem baixar:** abre `.cbz`/`.zip`/`.cbr`/`.rar` da nuvem página por página, em página única, página dupla ou rolagem contínua, da direita para a esquerda ou da esquerda para a direita, e continua de onde parou.
- **Ouvir OSTs:** cada álbum tem encarte com capa, faixas e discos. A barra de música fica embaixo da janela enquanto navega, com aleatório, repetir, volume igualado entre álbuns e as teclas de mídia do teclado.
- **Episódios com miniatura:** título, nota e miniatura de cada episódio pelo TMDB, em grade ou lista (opcional, veja abaixo).
- **Downloads:** episódio por episódio ou "Baixar todos", com fila (2 ao mesmo tempo) e progresso na barra de título.

## Requisitos

- Uma conta na Anitsu (o login é só via Discord e o cadastro de contas novas é feito pela própria Anitsu).
- Para desenvolver: [Rust](https://www.rust-lang.org/tools/install). No Windows, as ferramentas de build do Visual Studio (C++) e o Windows SDK, que o [GPUI](https://www.gpui.rs/) usa.

## Como rodar

### Chave do Supabase

O app precisa da chave `anon` pública que o próprio site da Anitsu usa no login. Por respeito aos desenvolvedores da Anitsu, ela **não fica neste repositório**: cada pessoa configura a sua localmente.

1. Copie `.env.example` para `.env` na raiz do projeto (o `.env` está no `.gitignore`).
2. Preencha `ANITSU_SUPABASE_ANON_KEY` com a chave. Também pode ser uma variável de ambiente com o mesmo nome.

No build, a chave é embutida no executável mascarada, sem aparecer em texto puro. Sem ela, o build para com uma mensagem explicando o que falta.

### Motor de vídeo (mpv)

O player de vídeo usa o [mpv](https://mpv.io/) (libmpv), carregado em tempo de execução e desenhando numa janela filha embaixo da interface GPUI. As DLLs não ficam no repositório. Ponha `libmpv-2.dll` em `crates/higanbana-desktop/lib/` (pasta no `.gitignore`) ou ao lado do executável. Use uma build **LGPL** da libmpv ([zhongfly/mpv-winbuild](https://github.com/zhongfly/mpv-winbuild)). Sem a DLL, o app abre normalmente e só o player avisa que falta o motor.

### Episódios (TMDB)

Na aba Episódios de um anime, cada arquivo de vídeo que casa com um episódio do [TMDB](https://www.themoviedb.org/) mostra miniatura, título em português (ou em inglês, quando não há tradução), data de exibição, nota e duração. O TMDB só é consultado quando você abre a página da obra, e as miniaturas carregam conforme aparecem na tela. Filmes e especiais de um episódio só aparecem como um card único com a imagem, o título, a nota e a duração do TMDB.

1. Crie uma conta no TMDB e gere as credenciais da API em *Configurações › API*.
2. No `.env` (copiado de `.env.example`), preencha `TMDB_TOKEN` com o *API Read Access Token* (v4, preferido) e, se quiser, `TMDB_KEY` com a *API Key* (v3, usada só quando não há token). Também valem variáveis de ambiente com os mesmos nomes.
3. Compile de novo: como a chave da Anitsu, as credenciais são embutidas no executável mascaradas.

Sem token o build continua e o app funciona normalmente: a aba mostra os mesmos cards com miniatura neutra e "Episódio N" (ou o nome do arquivo), sem nota. O mesmo acontece quando o TMDB está fora do ar ou não acha a obra.

O botão de ícone à direita, acima dos episódios, alterna entre **grade** (cards com miniatura, estilo Crunchyroll) e **lista** (miniatura, título, nota e sinopse curta). A escolha vale para todas as obras e fica salva.

### Comandos

```sh
cargo run -p higanbana-desktop              # abre o app
cargo build -p higanbana-desktop --release  # executável otimizado
cargo test -p higanbana-core                # testes do núcleo
```

O frontend web antigo (SolidJS, em `src/`) fica no repositório como base para as telas do app GPUI; ele não é mais compilado com o app.

## Como funciona

Dois crates num workspace Cargo:

- `crates/higanbana-core`: tudo que não depende de interface. Cria um `Core` com os serviços abaixo e avisa mudanças (sessão, downloads, música) por eventos.
- `crates/higanbana-desktop`: o app, feito com [GPUI](https://www.gpui.rs/) (a interface do Zed).

| Parte | Onde (`crates/higanbana-core/src/`) | O que faz |
| --- | --- | --- |
| Autenticação | `auth/` | Login OAuth (PKCE) com o Discord via navegador do sistema, renovação e logout da sessão. |
| HTTP | `http/` | Cliente com URL base e autenticação embutidas para os serviços da Anitsu. |
| Acervo | `nuvem/` | Listagem, busca e download em stream direto para o disco. |
| AniList | `anilist/` | Consultas em lote, casamento de nomes de pasta com títulos e cache em disco. |
| Player | `player/` | Proxy local de stream (Range → nuvem), AniSkip com cache e progresso/configurações em `player.json`. |
| Leitor | `reader/` | Abre `.cbz`/`.zip`/`.cbr`/`.rar` da nuvem por pedaços, página por página. |
| Música | `music/` | Lê as faixas por Range, decodifica, iguala o volume e toca, com teclas de mídia. |

Os dados ficam na mesma pasta que a versão Tauri usava (`dev.kingtimer12.higanbana`), então sessão, progresso e caches continuam valendo.

**Privacidade:** os tokens de sessão ficam só no núcleo e são gravados na pasta de dados do aplicativo no seu computador. Eles nunca são enviados a outro lugar além dos próprios servidores da Anitsu.

**Uso leve das APIs:** consultas ao AniList são agrupadas e guardadas em cache; a checagem de quais animes estão no acervo só acontece quando o card aparece na tela, com poucos pedidos em paralelo.

## Créditos

- [mpv](https://mpv.io/): motor de vídeo, usado como biblioteca dinâmica (build LGPL; licença do mpv: LGPL-2.1+ / GPL-2.0+ conforme a build).
- [GPUI](https://www.gpui.rs/) (Apache-2.0), do editor Zed.
- [AniSkip](https://aniskip.com/): tempos de abertura e encerramento enviados pela comunidade, usados quando o arquivo não tem capítulos.

## Licença

Distribuído sob a [licença MIT](LICENSE). A licença cobre apenas o código deste repositório, não o conteúdo do acervo nem os dados do AniList, que pertencem a quem os criou.
