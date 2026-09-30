import { fileURLToPath } from "node:url";

import tailwindcss from "@tailwindcss/vite";
import { tanstackRouter } from "@tanstack/router-plugin/vite";
import react from "@vitejs/plugin-react";
import { FileSystemIconLoader } from "unplugin-icons/loaders";
import Icons from "unplugin-icons/vite";
import { defineConfig, type Plugin } from "vite";
import wasm from "vite-plugin-wasm";

// https://vite.dev/config/
export default defineConfig({
  resolve: {
    tsconfigPaths: true,
  },
  plugins: [
    tanstackRouter({
      target: "react",
      autoCodeSplitting: true,
    }),
    tailwindcss(),
    react(),
    // Icons compile to inline React components at build time, so only the
    // glyphs actually imported ship. `figma` holds the exact exports from the
    // design file: the logo and the HUD icons, pixel for pixel.
    Icons({
      compiler: "jsx",
      jsx: "react",
      customCollections: {
        figma: FileSystemIconLoader(
          fileURLToPath(new URL("./src/assets/figma", import.meta.url)),
        ),
      },
    }),
    (wasm as unknown as () => Plugin)(),
  ],
  server: {
    proxy: {
      "/api": {
        target: "http://localhost:8080",
        changeOrigin: true,
        secure: false,
      },
    },
  },
});
