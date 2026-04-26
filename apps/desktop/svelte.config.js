import adapter from '@sveltejs/adapter-static';
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';

const config = {
  preprocess: vitePreprocess(),
  kit: {
    adapter: adapter({
      pages: 'build',
      assets: 'build',
      fallback: 'index.html',
      precompress: false,
      strict: true
    }),
    files: {
      assets: 'static',
      lib: 'src/lib',
      routes: 'src/routes',
      appTemplate: 'src/app.html'
    }
  }
};

export default config;
