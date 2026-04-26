import { defineConfig } from 'vite';
import { sveltekit } from '@sveltejs/kit/vite';

// Tauri runs the dev server on port 1420 with strictPort. HMR over websocket
// must be reachable from inside the Tauri webview.
export default defineConfig({
  plugins: [sveltekit()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: '127.0.0.1',
    hmr: {
      protocol: 'ws',
      host: '127.0.0.1',
      port: 1421
    },
    watch: {
      // Don't watch the Rust side — Tauri handles that.
      ignored: ['**/src-tauri/**']
    }
  },
  // Prevent Vite from obscuring Rust panics in stderr.
  envPrefix: ['VITE_', 'TAURI_']
});
