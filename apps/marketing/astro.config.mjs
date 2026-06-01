import { defineConfig } from 'astro/config';
import tailwind from '@astrojs/tailwind';

export default defineConfig({
  output: 'static',
  site: 'https://serpentine.dev',
  integrations: [tailwind({ applyBaseStyles: false })],
});
