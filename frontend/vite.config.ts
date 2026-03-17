import tailwindcss from '@tailwindcss/vite'
import react from '@vitejs/plugin-react'
import { resolve } from 'path'
import { defineConfig } from 'vite'

export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: {
      '@': resolve(__dirname, 'src'),
      '@domains': resolve(__dirname, 'src/domains'),
      '@app': resolve(__dirname, 'src/app'),
      '@ui': resolve(__dirname, 'src/ui'),
      '@store': resolve(__dirname, 'src/store.ts'),
    },
  },
  build: {
    outDir: '../src/serpentine/static',
    emptyOutDir: true,
  },
  base: './',
  server: {
    proxy: {
      '/api': 'http://localhost:8765',
      '/ws': { target: 'ws://localhost:8765', ws: true, changeOrigin: true, rewriteWsOrigin: true },
    },
  },
})
