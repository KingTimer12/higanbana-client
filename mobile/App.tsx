import { mount } from "nsh";
import { setBackend } from "./lib/backend";
import { nativeBackend } from "./lib/native";
import { Root } from "./Root";

setBackend(nativeBackend());
mount(Root);
