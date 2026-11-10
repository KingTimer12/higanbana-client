// O app com o acervo de mentira, sem sessão: começa na tela de login.
import { mount } from "nsh";
import { setBackend } from "./lib/backend";
import { Root } from "./Root";
import { fakeBackend } from "./fixture/fake";

setBackend(fakeBackend({ signedIn: false }));
mount(Root);
