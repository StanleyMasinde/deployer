import { ViteSSG } from "vite-ssg";
import App from "./App.vue";
import { routes, registerRouter } from "./router";
import "@fontsource/inter/latin-400.css";
import "@fontsource/inter/latin-500.css";
import "@fontsource/inter/latin-600.css";
import "@fontsource/space-grotesk/latin-500.css";
import "@fontsource/space-grotesk/latin-600.css";
import "@fontsource/jetbrains-mono/latin-400.css";
import "./styles.css";

export const createApp = ViteSSG(
  App,
  { routes, scrollBehavior: () => ({ top: 0 }) },
  ({ router }) => registerRouter(router),
);
