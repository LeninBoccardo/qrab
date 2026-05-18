/// <reference types="vitest" />
import { defineConfig } from "vite";
import solid from "vite-plugin-solid";
import tailwindcss from "@tailwindcss/vite";

// @ts-expect-error process is a nodejs global
const host = process.env.TAURI_DEV_HOST;
// @ts-expect-error process is a nodejs global
const isTest = !!process.env.VITEST;

// https://vite.dev/config/
export default defineConfig(async () => ({
    // solid-refresh (HMR) can't resolve under Vitest; disable it for tests.
    plugins: [solid({ hot: !isTest }), tailwindcss()],

    // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
    //
    // 1. prevent Vite from obscuring rust errors
    clearScreen: false,
    // 2. tauri expects a fixed port, fail if that port is not available
    server: {
        port: 1420,
        strictPort: true,
        host: host || false,
        hmr: host
            ? {
                  protocol: "ws",
                  host,
                  port: 1421,
              }
            : undefined,
        watch: {
            // 3. tell Vite to ignore watching `src-tauri`
            ignored: ["**/src-tauri/**"],
        },
    },

    // Vitest config — tests live alongside source as `*.test.ts(x)`.
    // jsdom + the global setup file enables both pure-function tests
    // and Solid component tests via @solidjs/testing-library.
    test: {
        environment: "jsdom",
        include: ["src/**/*.test.{ts,tsx}"],
        setupFiles: ["src/test/setup.ts"],
    },
}));
