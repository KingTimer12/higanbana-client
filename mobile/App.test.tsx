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
  await app.waitFor("Temporadas");
  expect(app.texts()).toContain("Spy x Family Season 2 (2023)");
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
  await app.waitFor("Mangás ainda não chegaram ao celular");

  // De volta aos animes, a letra escolhida continua lá.
  await app.press("id:tab-animes");
  await app.waitFor("Spy x Family");

  await app.press("id:tab-home");
  await app.waitFor("Continuar assistindo");
});
