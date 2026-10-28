import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: './tests/e2e',
  timeout: 30_000,
  fullyParallel: true,
  reporter: process.env.CI ? [['list'], ['html', { open: 'never' }]] : 'list',
  outputDir: 'test-results',
  use: {
    baseURL: 'http://127.0.0.1:1420',
    channel: 'chrome',
    headless: true,
    trace: 'retain-on-failure',
    screenshot: 'only-on-failure',
    viewport: { width: 1240, height: 820 },
  },
  webServer: {
    command: 'npm run dev:web -- --host 127.0.0.1',
    url: 'http://127.0.0.1:1420',
    reuseExistingServer: true,
    timeout: 60_000,
  },
});
