import { defineConfig, type Plugin } from "vite";
import vue from "@vitejs/plugin-vue";
import VueRouter from "vue-router/vite";
import { prerenderedRoutes } from "./src/router/ssg.ts";
import type { ViteSSGOptions } from "vite-ssg";

function spaShell(): Plugin {
  let ssr = false;

  return {
    name: "deployer-spa-shell",
    apply: "build",
    enforce: "post",

    configResolved(config) {
      ssr = Boolean(config.build.ssr);
    },

    generateBundle(_, bundle) {
      if (ssr) return;

      const index = bundle["index.html"];
      if (!index || index.type !== "asset") {
        throw new Error("Client build did not produce index.html");
      }

      this.emitFile({ type: "asset", fileName: "spa.html", source: index.source });
    },
  };
}

const ssgOptions: ViteSSGOptions = {
  dirStyle: "nested",
  includedRoutes: prerenderedRoutes,
};

export default defineConfig(({ command, mode }) => ({
  plugins: [VueRouter({ watch: command === "serve" && mode !== "test" }), vue(), spaShell()],
  optimizeDeps: { include: ["vue-router/experimental"] },
  server: {
    host: "127.0.0.1",
    port: 5173,
    strictPort: true,
    hmr: { host: "127.0.0.1", port: 5173 },
  },
  ssgOptions,
}));
