import { expect, test } from '@playwright/test';
import { snapshot, update } from './fixtures.js';
import { installMock } from './mock.js';

async function open(page) {
  await installMock(page, { snapshot, update });
  await page.goto('/');
  await expect(page.getByRole('heading', { name: 'Library', exact: true })).toBeVisible();
}

test('library structure', async ({ page }) => {
  await open(page);
  await expect(page.locator('.page')).toMatchAriaSnapshot();
});

test('game structure', async ({ page }) => {
  await open(page);
  await page.locator('[data-card] .title').first().click();
  await expect(page.getByRole('heading', { name: 'Echoes of the Crown' })).toBeVisible();
  await expect(page.locator('.page')).toMatchAriaSnapshot();
});

test('settings structure', async ({ page }) => {
  await open(page);
  await page.locator('[data-nav="settings"]').click();
  await expect(page.getByRole('heading', { name: 'Settings' })).toBeVisible();
  await expect(page.locator('.page')).toMatchAriaSnapshot();
});
