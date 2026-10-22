import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: { alias: { "@": new URL("./src", import.meta.url).pathname } },
  server: {
    port: 1420,
    strictPort: true,
    // The UI reuses the native icon from the repository, not a duplicate asset.
    fs: { allow: [new URL("..", import.meta.url).pathname] },
  },
  clearScreen: false,
});
