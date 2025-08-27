import { defineConfig } from 'astro/config';
import tailwind from '@astrojs/tailwind';
import react from '@astrojs/react';

// Если сайт будет размещён по адресу https://<user>.github.io/TRIAD/
// раскомментируйте base: '/TRIAD'
export default defineConfig({
  integrations: [tailwind({ applyBaseStyles: false }), react()],
  // Full production URL of the site
  site: 'https://fillay12321.github.io/TRIAD',
  // Base path equals the repository name for GitHub Pages project sites
  base: '/TRIAD',
  scopedStyleStrategy: 'where',
});
