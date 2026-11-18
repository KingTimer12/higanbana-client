// As telas, de ponta a ponta, sobre o acervo de mentira (`fixture/fake.ts`).
import { expect, open, test } from "nsh/test";
import { highlightsHeading } from "./lib/format";

test("quem não tem sessão entra pelo Discord e chega ao início", async () => {
  const app = await open("./FixtureSignedOut.tsx");
  await app.waitFor("Entrar com Discord");
  await app.press("Entrar com Discord");
  await app.waitFor(highlightsHeading());
  expect(app.texts()).toContain("Olá, Aaron");
});

test("o início mostra os destaques, o que continuar e o descobrir", async () => {
  const app = await open("./Fixture.tsx");
  await app.waitFor(highlightsHeading());
  await app.waitFor("Continuar assistindo");
  await app.waitFor("Descobrir");
  // Os destaques só trazem o que está no acervo.
  await app.waitFor("id:highlight-1");
  expect(app.texts()).not.toContain("Fora do Acervo");
});

test("o descobrir troca de lista", async () => {
  const app = await open("./Fixture.tsx");
  await app.waitFor("Mais bem avaliados");
  await app.press("id:discover-trending");
  await app.waitFor("label:Bocchi the Rock!");
});

test("do destaque à obra: versões, episódios e o player", async () => {
  const app = await open("./Fixture.tsx");
  await app.waitFor("id:highlight-1");
  await app.press("id:highlight-1");
  await app.waitFor("Episódios");
  await app.waitFor("Episódio 2");
  expect(app.texts()).toContain("Versão");
  expect(app.texts()).toContain("Continuar episódio 1");

  await app.press("Episódio 2");
  await app.waitFor("label:Pausar");
  expect(app.texts()).toContain("Episódio 2");

  await app.press("label:Pausar");
  await app.waitFor("label:Tocar");

  await app.press("label:Próximo episódio");
  await app.waitFor("Episódio 3");
});

test("o catálogo lista as obras da letra escolhida", async () => {
  const app = await open("./Fixture.tsx");
  await app.waitFor("id:tab-animes");
  await app.press("id:tab-animes");
  await app.press("id:letter-S");
  await app.waitFor("Spy x Family");
  await app.press("Spy x Family");
  await app.waitFor("Franquia · 2022–2023");
});

test("a franquia separa as obras em abas, como no desktop", async () => {
  const app = await open("./Fixture.tsx");
  await app.press("id:tab-animes");
  await app.press("id:letter-S");
  await app.waitFor("Spy x Family");
  await app.press("Spy x Family");
  // O filme casa com um MOVIE do AniList e vai para a aba Filmes.
  await app.waitFor("Séries TV · 2");
  await app.waitFor("Filmes · 1");
  expect(app.texts()).toContain("Spy x Family Season 2");
  expect(app.texts()).not.toContain("Spy x Family Code White");

  await app.press("id:franchise-movies");
  await app.waitFor("Spy x Family Code White");
  expect(app.texts()).not.toContain("Spy x Family Season 2");

  // Cada obra abre como uma obra comum.
  await app.press("id:work-[2023.1] Spy x Family Code White [FILME]");
  await app.waitFor("Episódios");
});

test("o filtro do catálogo restringe a letra", async () => {
  const app = await open("./Fixture.tsx");
  await app.press("id:tab-animes");
  await app.press("id:letter-S");
  await app.waitFor("Spy x Family");
  await app.type("label:Filtrar títulos", "zzz");
  await app.waitFor("Nenhum título com “zzz” em S");
});

test("os episódios trazem o título do TMDB quando casam", async () => {
  const app = await open("./Fixture.tsx");
  await app.waitFor("id:highlight-1");
  await app.press("id:highlight-1");
  await app.waitFor("O fim da jornada");
  expect(app.texts()).toContain("24 min");
  // Sem episódio casado, o rótulo do arquivo, como no desktop.
  expect(app.texts()).toContain("Episódio 2");
});

test("a barra de abas troca de seção e guarda o estado da aba", async () => {
  const app = await open("./Fixture.tsx");
  await app.waitFor("id:tab-animes");
  await app.press("id:tab-animes");
  await app.press("id:letter-S");
  await app.waitFor("Spy x Family");

  await app.press("id:tab-mangas");
  await app.waitFor("id:letter-C");

  // De volta aos animes, a letra escolhida continua lá.
  await app.press("id:tab-animes");
  await app.waitFor("Spy x Family");

  await app.press("id:tab-home");
  await app.waitFor("Continuar assistindo");
});

test("a tela de login mostra a versão instalada", async () => {
  const app = await open("./FixtureSignedOut.tsx");
  await app.waitFor("Versão 0.1.0");
});

test("sem versão nova, o início não mostra aviso", async () => {
  const app = await open("./Fixture.tsx");
  await app.waitFor(highlightsHeading());
  expect(app.texts()).not.toContain("Versão 0.2.0 disponível");
});

test("a versão nova aparece no início, baixa e abre o instalador", async () => {
  const app = await open("./FixtureUpdate.tsx");
  await app.waitFor("Versão 0.2.0 disponível");
  await app.press("Atualizar");
  await app.waitFor("Pronta para instalar.");
  expect(app.texts()).not.toContain("Não foi possível abrir o instalador.");
});

test("o aviso de versão nova pode ser fechado", async () => {
  const app = await open("./FixtureUpdate.tsx");
  await app.waitFor("Versão 0.2.0 disponível");
  await app.press("label:Agora não");
  await app.waitFor(highlightsHeading());
  expect(app.texts()).not.toContain("Versão 0.2.0 disponível");
});

test("os mangás usam o mesmo catálogo, com a busca da seção", async () => {
  const app = await open("./Fixture.tsx");
  await app.press("id:tab-mangas");
  await app.press("id:letter-C");
  await app.waitFor("Chainsaw Man");
  // Só mangás: os animes da letra não aparecem aqui.
  await app.press("id:letter-S");
  await app.waitFor("Nenhuma obra nesta letra");

  await app.press("label:Buscar mangá");
  await app.type("label:Buscar mangá", "yotsuba");
  await app.waitFor("Yotsuba to!");
});

test("do mangá aos capítulos e ao leitor, que guarda onde parou", async () => {
  const app = await open("./Fixture.tsx");
  await app.press("id:tab-mangas");
  await app.press("id:letter-C");
  await app.waitFor("Chainsaw Man");
  await app.press("Chainsaw Man");
  await app.waitFor("Capítulos");
  await app.waitFor("Ler Cap. 1");
  expect(app.texts().some((t) => t.includes("97 capítulos"))).toBe(true);

  await app.press("id:chapter-Cap 01.cbz");
  await app.waitFor("Próximo: Cap. 2");
  await app.press("label:Próximo capítulo");
  await app.waitFor("Próximo: Cap. 3");

  await app.press("label:Fechar o leitor");
  // Seguir adiante conta o capítulo como lido; o botão já oferece o seguinte.
  await app.waitFor("Lido");
  await app.waitFor("Ler Cap. 2");
});

test("a coletânea lê os capítulos de dentro do arquivo em sequência", async () => {
  const app = await open("./Fixture.tsx");
  await app.press("id:tab-mangas");
  await app.press("id:letter-Y");
  await app.waitFor("Yotsuba to!");
  await app.press("Yotsuba to!");
  await app.waitFor("Ler Caps. 1-3");
  await app.press("Ler Caps. 1-3");
  await app.waitFor("Cap. 1");
  await app.press("label:Próximo capítulo");
  await app.waitFor("Cap. 2");
});
