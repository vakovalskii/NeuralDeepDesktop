import { defineConfig, loadEnv } from "vite";
import react from "@vitejs/plugin-react";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

// Read the Hermes loopback API key from ~/.hermes/.env so the browser frontend
// never has to hold it. The Vite dev server proxies /hermes/* → Hermes :8642
// and injects the Authorization header server-side.
function readHermesKey(): string {
  if (process.env.API_SERVER_KEY) return process.env.API_SERVER_KEY;
  try {
    const envPath = path.join(os.homedir(), ".hermes", ".env");
    const txt = fs.readFileSync(envPath, "utf8");
    const m = txt.match(/^API_SERVER_KEY=(.*)$/m);
    if (m) return m[1].trim();
  } catch {
    /* ignore */
  }
  return "";
}

export default defineConfig(({ mode }) => {
  loadEnv(mode, process.cwd(), "");
  const HERMES = process.env.HERMES_URL || "http://127.0.0.1:8642";
  const KEY = readHermesKey();

  return {
    plugins: [react()],
    clearScreen: false,
    server: {
      host: "127.0.0.1",
      port: 5173,
      strictPort: true,
      proxy: {
        "/hermes": {
          target: HERMES,
          changeOrigin: true,
          rewrite: (p) => p.replace(/^\/hermes/, ""),
          configure: (proxy) => {
            proxy.on("proxyReq", (proxyReq) => {
              if (KEY) proxyReq.setHeader("Authorization", `Bearer ${KEY}`);
              // The dev proxy is the trusted loopback client. Drop the browser
              // Origin so Hermes treats it as a non-browser caller (no CORS gate).
              proxyReq.removeHeader("origin");
            });
          },
        },
      },
    },
  };
});
