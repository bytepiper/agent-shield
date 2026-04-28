import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import { defineConfig } from "vite";

const dashboardApiTarget =
  process.env.AGENT_SHIELD_DASHBOARD_API_TARGET ?? "http://127.0.0.1:9999";

export default defineConfig({
  plugins: [react(), tailwindcss()],
  server: {
    host: "0.0.0.0",
    port: 4173,
    proxy: {
      "/api": {
        target: dashboardApiTarget,
        changeOrigin: true,
      },
    },
  },
  preview: {
    host: "0.0.0.0",
    port: 4173,
  },
});
