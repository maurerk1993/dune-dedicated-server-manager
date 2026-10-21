import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    strictPort: true,
  },
  envPrefix: ["VITE_", "TAURI_"],
  // Preserve the Vite 5 browser baseline when upgrading the build toolchain.
  build: {
    target: ["es2020", "edge88", "firefox78", "chrome87", "safari14"],
  },
});
