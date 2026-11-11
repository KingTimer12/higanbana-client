// O app com o acervo de mentira e já logado: `nsh run Fixture.tsx`.
import { mount } from "nsh";
import { setBackend } from "./lib/backend";
import { Root } from "./Root";
import { fakeBackend } from "./fixture/fake";

setBackend(fakeBackend({ signedIn: true }));
mount(Root);
