import { ViteSSG } from "vite-ssg";
import App from "./App.vue";
import { routes, registerRouter } from "./router";
import "@fontsource/manrope/latin-400.css";
import "@fontsource/manrope/latin-500.css";
import "@fontsource/manrope/latin-600.css";
import "@fontsource/manrope/latin-700.css";
import "@fontsource/ibm-plex-mono/latin-400.css";
import "./styles.css";

export const createApp = ViteSSG(
  App,
  { routes, scrollBehavior: () => ({ top: 0 }) },
  ({ router }) => registerRouter(router),
);
