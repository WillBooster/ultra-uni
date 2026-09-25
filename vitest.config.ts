import { defineConfig } from 'vitest/config';

export default defineConfig({
  test: {
    // `bun wb test` and `bun wb verify` start vitest directly, so the wasm build must run here.
    globalSetup: ['./test/helpers/globalSetup.ts'],
  },
});
