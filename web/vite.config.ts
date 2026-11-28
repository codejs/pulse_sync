import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";

// The dev server proxies /api to the daemon and attaches its API token, read
// from the daemon's data dir (the same one `just dev` passes to pulsed).
const daemonUrl = process.env.PULSE_DAEMON_URL ?? "http://127.0.0.1:8417";
const dataDir = resolve(__dirname, process.env.PULSE_DATA_DIR ?? "../.pulse-dev");

function apiToken(): string | undefined {
  try {
    return readFileSync(resolve(dataDir, "api-token"), "utf8").trim();
  } catch {
    return undefined; // daemon not started yet; requests will get 401
  }
}

export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: { "@": resolve(__dirname, "src") },
  },
  server: {
    port: 5173,
    strictPort: true,
    proxy: {
      "/api": {
        target: daemonUrl,
        changeOrigin: true,
        ws: true,
        configure: (proxy) => {
          const auth = (req: { setHeader(name: string, value: string): void }) => {
            const token = apiToken();
            if (token) req.setHeader("Authorization", `Bearer ${token}`);
          };
          proxy.on("proxyReq", auth);
          proxy.on("proxyReqWs", auth);
        },
      },
    },
  },
  test: {
    environment: "jsdom",
  },
});
