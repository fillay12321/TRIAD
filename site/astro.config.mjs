import { defineConfig } from 'astro/config';
import tailwind from '@astrojs/tailwind';
import react from '@astrojs/react';

// Если сайт будет размещён по адресу https://<user>.github.io/TRIAD/
// раскомментируйте base: '/TRIAD'
export default defineConfig({
  integrations: [tailwind({ applyBaseStyles: false }), react()],
  site: 'https://fillay12321.github.io',
  base: '/triad-site',
  scopedStyleStrategy: 'where',
});
