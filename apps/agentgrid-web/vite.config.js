import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

export default defineConfig({
  base: '/agentgrid/',
  plugins: [react()],
  build: {
    rollupOptions: {
      output: {
        manualChunks(id) {
          if (!id.includes('node_modules')) return undefined;
          if (id.includes('/react/') || id.includes('/react-dom/') || id.includes('/scheduler/')) {
            return 'vendor-react';
          }
          if (id.includes('/antd/') || id.includes('/@ant-design/icons/')) {
            return 'vendor-antd';
          }
          if (id.includes('/@ant-design/pro-components/')) {
            return 'vendor-pro';
          }
          return 'vendor';
        },
      },
    },
    chunkSizeWarningLimit: 900,
  },
});
