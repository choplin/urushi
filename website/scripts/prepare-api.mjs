import { spawnSync } from 'node:child_process';
import { cpSync, mkdirSync, rmSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const websiteDirectory = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const repositoryRoot = resolve(websiteDirectory, '..');
const rustdocDirectory = resolve(repositoryRoot, 'target', 'doc');
const publicDirectory = resolve(websiteDirectory, 'public');
const generatedDirectory = resolve(websiteDirectory, '.generated-public');
const generatedApiDirectory = resolve(generatedDirectory, 'api');

const result = spawnSync(
  'cargo',
  ['doc', '--workspace', '--no-deps'],
  { cwd: repositoryRoot, stdio: 'inherit' },
);

if (result.error) {
  throw result.error;
}
if (result.status !== 0) {
  process.exit(result.status ?? 1);
}

rmSync(generatedDirectory, { recursive: true, force: true });
mkdirSync(generatedDirectory, { recursive: true });
cpSync(publicDirectory, generatedDirectory, { recursive: true });
cpSync(rustdocDirectory, generatedApiDirectory, { recursive: true });
