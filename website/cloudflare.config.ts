import { defineConfig } from 'cf/config';
import { workerConfig } from './scripts/cloudflare-worker.mjs';

export default defineConfig({
  worker: workerConfig,
});
