import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: './tests',
  outputDir: '../../../test-artifacts/nonstop-notify',
  reporter: 'line',
  use: {
    baseURL: 'http://127.0.0.1:1420',
    browserName: 'chromium',
    headless: true,
    viewport: { width: 430, height: 760 },
  },
  webServer: {
    command: 'npm run dev:notify-ui',
    cwd: '../..',
    url: 'http://127.0.0.1:1420',
    reuseExistingServer: true,
  },
});
