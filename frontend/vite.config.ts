import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  server: {
    port: 9131,
    strictPort: true,
    hmr: {
      port: 9132,
    },
  },
  build: {
    outDir: "dist",
    emptyOutDir: true,
  },
});
