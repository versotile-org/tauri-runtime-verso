import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

// https://vitejs.dev/config/
export default defineConfig({
	root: 'src',
	plugins: [react()],
	server: {
		port: 1420,
		strictPort: true,
	},
	build: {
		outDir: '../dist',
		emptyOutDir: true,
		target: 'esnext',
		modulePreload: false,
	},
})
