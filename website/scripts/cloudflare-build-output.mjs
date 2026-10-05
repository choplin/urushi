import { cp, mkdir, rm, writeFile } from 'node:fs/promises';

import { workerConfig } from './cloudflare-worker.mjs';

const buildOutputRoot = new URL('../.cloudflare/output/v0/', import.meta.url);
const workerOutput = new URL('workers/default/', buildOutputRoot);

export function cloudflareBuildOutput() {
  return {
    name: 'urushi-cloudflare-build-output',
    hooks: {
      'astro:build:done': async ({ dir }) => {
        await rm(buildOutputRoot, { recursive: true, force: true });
        await mkdir(workerOutput, { recursive: true });
        await cp(dir, new URL('assets/', workerOutput), { recursive: true });
        await writeJson(
          new URL('config.json', buildOutputRoot),
          { buildContext: { isPreview: false, mode: 'production' } },
        );
        await writeJson(
          new URL('worker.config.json', workerOutput),
          workerConfig,
        );
      },
    },
  };
}

async function writeJson(url, value) {
  await writeFile(url, `${JSON.stringify(value)}\n`);
}
