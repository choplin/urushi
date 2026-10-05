import { mkdir } from 'node:fs/promises';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import sharp from 'sharp';

const websiteDirectory = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const repositoryRoot = resolve(websiteDirectory, '..');
const sourceIcon = resolve(repositoryRoot, 'assets', 'brand', 'urushi-icon.png');
const publicIcon = resolve(websiteDirectory, 'public', 'brand', 'urushi-icon.png');

await mkdir(dirname(publicIcon), { recursive: true });
await sharp(sourceIcon)
  .resize(224, 224, { fit: 'contain' })
  .png({ compressionLevel: 9 })
  .toFile(publicIcon);
