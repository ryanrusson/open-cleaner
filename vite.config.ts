import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

export default defineConfig({
  plugins: [react()],
  // Tauri expects a fixed port and prints its own output.
  clearScreen: false,
  server: { port: 1420, strictPort: true },
});
