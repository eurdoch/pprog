import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

// https://vite.dev/config/
export default defineConfig({
  plugins: [react()],
  build: {
    // Increase chunk size warning limit to 1MB
    chunkSizeWarningLimit: 1000,
    rollupOptions: {
      output: {
        // Manually split chunks to optimize bundle size
        manualChunks(id) {
          // Split large node_modules dependencies into separate chunks
          if (id.includes('node_modules')) {
            // Chunk common library dependencies
            if (
              id.includes('react') || 
              id.includes('react-dom') || 
              id.includes('@emotion') || 
              id.includes('styled-components')
            ) {
              return 'vendor';
            }
          }
        }
      }
    }
  }
})