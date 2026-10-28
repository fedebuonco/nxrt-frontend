import { expect, test } from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';
import { snapshot, update } from './fixtures.js';
import { installMock } from './mock.js';

async function open(page) {
  await installMock(page, { snapshot, update });
  await page.goto('/');
  await expect(page.getByRole('heading', { name: 'Library', exact: true })).toBeVisible();
  await page.addStyleTag({ content: '*,*::before,*::after{animation:none!important;transition:none!important}' });
}

async function serious(page) {
  const r = await new AxeBuilder({ page }).withTags(['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa']).analyze();
  return r.violations
    .filter((v) => v.impact === 'serious' || v.impact === 'critical')
    .map((v) => ({ id: v.id, impact: v.impact, help: v.help, nodes: v.nodes.map((n) => n.target) }));
}

test('the library has no serious accessibility violations', async ({ page }) => {
  await open(page);
  expect(await serious(page), 'library accessibility').toEqual([]);
});

test('a game page has no serious accessibility violations', async ({ page }) => {
  await open(page);
  await page.locator('[data-card] .title').first().click();
  await expect(page.getByRole('heading', { name: 'Echoes of the Crown' })).toBeVisible();
  expect(await serious(page), 'game accessibility').toEqual([]);
});

test('settings has no serious accessibility violations', async ({ page }) => {
  await open(page);
  await page.locator('[data-nav="settings"]').click();
  await expect(page.getByRole('heading', { name: 'Settings' })).toBeVisible();
  expect(await serious(page), 'settings accessibility').toEqual([]);
});
