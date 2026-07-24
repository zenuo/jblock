import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// https://vitejs.dev/config/
// VITE_BASE is set in GitHub Actions Pages builds (e.g. "/jblock/").
export default defineConfig({
  base: process.env.VITE_BASE || "/",
  plugins: [react()],
  server: {
    host: true,
    port: 5173,
  },
});
