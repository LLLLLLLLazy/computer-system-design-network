import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';

const statusPort = process.env.STATUS_PORT || '5174';
const devPort = Number(process.env.PORT) || 5173;

export default defineConfig({
  plugins: [sveltekit()],
  server: {
    port: devPort,
    strictPort: true,
    proxy: {
      '/api': {
        target: `http://localhost:${statusPort}`,
        changeOrigin: true,
        secure: false,
        rewrite: (path) => path.replace(/^\/api/, '/api')
      }
    }
  }
});
