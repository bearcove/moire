import { defineConfig } from "vite";
import preact from "@preact/preset-vite";

export default defineConfig({
  plugins: [preact()],
  server: {
    port: 9131,
    proxy: {
      "/api": {
        target: "http://127.0.0.1:9130",
      },
      "/health": {
        target: "http://127.0.0.1:9130",
      },
    },
  },
  build: {
    outDir: "dist",
    emptyOutDir: true,
  },
});
