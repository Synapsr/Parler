import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

// Tauri expects a fixed port and its own error output.
export default defineConfig({
  plugins: [react(), tailwindcss()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: { ignored: ["**/src-tauri/**"] },
  },
  build: {
    // Two windows, two pages: the main window and the dictation pill.
    rollupOptions: {
      input: { main: "index.html", overlay: "overlay.html" },
    },
  },
  test: {
    include: ["src/**/*.test.ts"],
  },
});
