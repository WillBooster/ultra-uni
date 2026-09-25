import { defineConfig } from 'vitest/config';

export default defineConfig({
  test: {
    // Build the wasm package before every run, whichever command starts vitest.
    globalSetup: ['./test/helpers/globalSetup.ts'],
  },
});
