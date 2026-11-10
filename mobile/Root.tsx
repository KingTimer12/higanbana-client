// A casca do app: decide entre login e as telas, e guarda o que fica sobre todas elas.
import { View } from "nsh";
import { NavStack } from "nsh/base";
import { Notices } from "./components/Notices";
import { Loading } from "./components/ui";
import { Catalog } from "./pages/Catalog";
import { Home } from "./pages/Home";
import { Login } from "./pages/Login";
import { Player } from "./pages/Player";
import { Search } from "./pages/Search";
import { Title } from "./pages/Title";
import { nav, restoreSession, type Route, sessionState } from "./store";

function page(route: Route) {
  switch (route.name) {
    case "catalog":
      return <Catalog />;
    case "search":
      return <Search />;
    case "title":
      return <Title path={route.path} media={route.media} />;
    case "player":
      return <Player dir={route.dir} file={route.file} title={route.title} malId={route.malId} />;
    default:
      return <Home />;
  }
}

export function Root() {
  restoreSession();
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
