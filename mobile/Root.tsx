// A casca do app: decide entre login e as telas, e guarda o que fica sobre todas elas.
import { View } from "nsh";
import { NavStack } from "nsh/base";
import { Notices } from "./components/Notices";
import { Loading } from "./components/ui";
import { Downloads } from "./pages/Downloads";
import { Login } from "./pages/Login";
import { Player } from "./pages/Player";
import { Reader } from "./pages/Reader";
import { Search } from "./pages/Search";
import { Tabs } from "./pages/Tabs";
import { Title } from "./pages/Title";
import { sectionOf } from "./lib/sections";
import { checkForUpdate, nav, restoreSession, type Route, sessionState } from "./store";

function page(route: Route) {
  switch (route.name) {
    case "search":
      return <Search section={sectionOf(route.kind)} />;
    case "title":
      return <Title path={route.path} media={route.media} kind={route.kind} />;
    case "reader":
      return <Reader file={route.file} files={route.files} title={route.title} />;
    case "downloads":
      return <Downloads />;
    case "player":
      return <Player dir={route.dir} file={route.file} title={route.title} malId={route.malId} />;
    default:
      return <Tabs />;
  }
}

export function Root() {
  restoreSession();
  // Uma consulta por abertura; sem rede, segue calado.
  checkForUpdate();
  return (
    <View class="flex-1 bg-background">
      <If when={sessionState().status === "loading"}>
        <Loading label="Abrindo" />
      </If>
      <If when={sessionState().status === "signedOut"}>
        <Login />
      </If>
      <If when={sessionState().status === "signedIn"}>
        <NavStack state={nav} class="flex-1">
          {(route) => page(route)}
        </NavStack>
      </If>
      <Notices />
    </View>
  );
}
