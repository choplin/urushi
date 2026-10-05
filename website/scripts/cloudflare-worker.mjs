/** @satisfies {import('cf/config').WorkerConfig} */
export const workerConfig = {
  name: 'urushi',
  compatibilityDate: '2026-09-28',
  domains: ['urushi.choplin.dev'],
  workersDev: false,
  assets: {
    notFoundHandling: '404-page',
  },
};
