// As telas, de ponta a ponta, sobre o acervo de mentira (`fixture/fake.ts`).
import { expect, open, test } from "nsh/test";
import { seasonHeading } from "./lib/format";

test("quem não tem sessão entra pelo Discord e chega ao início", async () => {
  const app = await open("./FixtureSignedOut.tsx");
  await app.waitFor("Entrar com Discord");
  await app.press("Entrar com Discord");
  await app.waitFor(seasonHeading());
  expect(app.texts()).toContain("Olá, Aaron");
});

test("o início mostra o que continuar e a temporada", async () => {
  const app = await open("./Fixture.tsx");
  await app.waitFor("Continuar assistindo");
  await app.waitFor("Sousou no Frieren");
  expect(app.texts()).toContain("Dandadan");
});

test("um anime fora do acervo avisa em vez de abrir", async () => {
  const app = await open("./Fixture.tsx");
  await app.waitFor("Fora do Acervo");
  await app.press("label:Fora do Acervo");
  await app.waitFor("“Fora do Acervo” ainda não está na nuvem.");
});

test("da temporada à obra: versões, episódios e o player", async () => {
  const app = await open("./Fixture.tsx");
  await app.waitFor("Sousou no Frieren");
  await app.press("label:Sousou no Frieren");
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
  await app.waitFor("id:section-animes");
  await app.press("id:section-animes");
  await app.press("id:letter-S");
  await app.waitFor("Spy x Family");
  await app.press("Spy x Family");
  await app.waitFor("Temporadas");
  expect(app.texts()).toContain("Spy x Family Season 2 (2023)");
});

test("a temporada conta o que está no acervo", async () => {
  const app = await open("./Fixture.tsx");
  await app.waitFor("5 de 6 no acervo");
  expect(app.texts()).toContain(seasonHeading());
});

test("o filtro do catálogo restringe a letra", async () => {
  const app = await open("./Fixture.tsx");
  await app.press("id:section-animes");
  await app.press("id:letter-S");
  await app.waitFor("Spy x Family");
  await app.type("label:Filtrar títulos", "zzz");
  await app.waitFor("Nenhum título com “zzz” em S");
});

test("os episódios trazem o título do TMDB quando casam", async () => {
  const app = await open("./Fixture.tsx");
  await app.waitFor("Sousou no Frieren");
  await app.press("label:Sousou no Frieren");
  await app.waitFor("O fim da jornada");
  expect(app.texts()).toContain("24 min");
  // Sem episódio casado, o rótulo do arquivo, como no desktop.
  expect(app.texts()).toContain("Episódio 2");
});
