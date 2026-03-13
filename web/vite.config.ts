import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'

export default defineConfig({
  plugins: [vue()],
  build: {
    outDir: 'dist',
  },
  server: {
    port: 5173,
    proxy: {
      '/api': 'http://127.0.0.1:13100',
      '/mcp': 'http://127.0.0.1:13100',
      '/health': 'http://127.0.0.1:13100',
    },
  },
})
