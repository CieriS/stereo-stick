import { defineConfig } from "vite";

// The port is fixed because the desktop shell opens exactly this address.
export default defineConfig({
  clearScreen: false,
  server: { port: 4340, strictPort: true },
  build: { target: "es2022" },
});
