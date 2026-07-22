import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

const __dirname = path.dirname(fileURLToPath(import.meta.url));

export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      '@clsc/ui': path.resolve(__dirname, '../../packages/ui/src/index.tsx'),
      '@clsc/player': path.resolve(
        __dirname,
        '../../packages/player/src/index.tsx',
      ),
      '@clsc/api-client': path.resolve(
        __dirname,
        '../../packages/api-client/src/index.ts',
      ),
    },
  },
});
