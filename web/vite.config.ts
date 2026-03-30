import { defineConfig } from "vite";
import solid from "vite-plugin-solid";
import tailwindcss from "@tailwindcss/vite";

export default defineConfig({
  plugins: [tailwindcss(), solid()],
  server: {
    port: 5173,
    proxy: {
      "/api": "http://127.0.0.1:9100",
      "/ws": {
        target: "ws://127.0.0.1:9100",
        ws: true,
      },
    },
  },
  build: {
    outDir: "dist",
    target: "esnext",
  },
});
