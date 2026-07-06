import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
var host = process.env.TAURI_DEV_HOST;
export default defineConfig({
    plugins: [react()],
    clearScreen: false,
    server: {
        port: 3000,
        strictPort: true,
        host: host || false,
        hmr: host ? { protocol: "ws", host: host, port: 3001 } : undefined,
        watch: { ignored: ["**/src-tauri/**"] },
    },
    envPrefix: ["VITE_", "TAURI_ENV_*"],
    build: {
        target: "es2021",
        minify: !process.env.TAURI_ENV_DEBUG ? "esbuild" : false,
        sourcemap: !!process.env.TAURI_ENV_DEBUG,
    },
});
