// O app com o acervo de mentira, logado e com uma versão nova publicada.
import { mount } from "nsh";
import { setBackend } from "./lib/backend";
import { Root } from "./Root";
import { fakeBackend } from "./fixture/fake";

setBackend(fakeBackend({ signedIn: true, update: true }));
mount(Root);
