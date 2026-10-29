import { expect, test } from '@playwright/test';
import { releaseUrl, snapshot, update } from './fixtures.js';
import { installMock } from './mock.js';

test.beforeEach(async ({ page }) => {
  await installMock(page, { snapshot, update });
  await page.goto('/');
  await expect(page.getByRole('heading', { name: 'Library' })).toBeVisible();
});

test('opens a game and files an issue', async ({ page }) => {
  await page.locator('[data-card] .title').first().click();
  await expect(page.getByRole('heading', { name: 'Echoes of the Crown' })).toBeVisible();
  await expect(page.getByRole('button', { name: /Report an issue/ })).toBeVisible();

  await page.getByRole('button', { name: /Report an issue/ }).click();
  await expect.poll(() => page.evaluate(() => window.__calls.some((c) => c.cmd === 'open_issue'))).toBe(true);
  const call = await page.evaluate(() => window.__calls.find((c) => c.cmd === 'open_issue'));
  expect(call.args.key).toBe(snapshot.games[0].key);
});

test('the controller map test mode lights a pressed button', async ({ page }) => {
  await page.locator('[data-nav="settings"]').click();
  await expect(page.getByText('Controller map', { exact: true })).toBeVisible();

  await page.getByRole('button', { name: 'Test', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Stop', exact: true })).toBeVisible();
  await expect(page.locator('.pad-test-caption')).toContainText('Press any button');

  await page.evaluate(() => window.__emit('pad-button', { button: 'south', pressed: true }));
  const south = page.locator('.controller-map [data-host="south"]');
  await expect(south).toHaveClass(/on/);
  await expect(south).toHaveClass(/blink/);
  await expect(south.locator('.pad-map')).not.toBeEmpty();
  await expect(page.locator('.pad-test-caption')).toContainText('Bottom (A)');

  await page.evaluate(() => window.__emit('pad-button', { button: 'south', pressed: false }));
  await expect(south).not.toHaveClass(/on/);

  await page.keyboard.press('Escape');
  await expect(page.getByRole('button', { name: 'Test', exact: true })).toBeVisible();
});

test('the update chip opens the release page', async ({ page }) => {
  const chip = page.getByText('Update available');
  await expect(chip).toBeVisible({ timeout: 5_000 });
  await chip.click();

  await expect
    .poll(() => page.evaluate(() => window.__calls.find((c) => c.cmd === 'open_update')?.args?.url))
    .toBe(releaseUrl);
});

test('the donate link opens the support page', async ({ page }) => {
  await page.locator('[data-act="donate"]').click();
  await expect.poll(() => page.evaluate(() => window.__calls.some((c) => c.cmd === 'open_donate'))).toBe(true);
});

test('game art tilts and lights up under the pointer', async ({ page }) => {
  const cover = page.locator('.cover').first();
  await cover.hover({ position: { x: 20, y: 20 } });
  await expect(cover).toHaveClass(/holo-on/);
  const vars = await cover.evaluate((el) => ({
    rx: el.style.getPropertyValue('--rx'),
    ry: el.style.getPropertyValue('--ry'),
    mx: el.style.getPropertyValue('--mx'),
  }));
  expect(vars.rx).not.toBe('');
  expect(vars.ry).not.toBe('0.00deg');
  expect(vars.mx).not.toBe('');
  await page.mouse.move(1, 1);
  await expect(cover).not.toHaveClass(/holo-on/);
});

test('focused game art tilts and lights up', async ({ page }) => {
  const cover = page.locator('.cover').first();
  await page.locator('.game-card button.title').first().focus();
  await page.waitForTimeout(250);
  const t = await cover.evaluate((el) => getComputedStyle(el).transform);
  expect(t).not.toBe('none');
  const sheen = await cover.evaluate((el) => getComputedStyle(el, '::after').opacity);
  expect(Number(sheen)).toBeGreaterThan(0);
});

test.describe('reduced motion', () => {
  test.use({ reducedMotion: 'reduce' });

  test('still shows the static foil and pointer tilt', async ({ page }) => {
    await page
      .locator('.cover')
      .first()
      .hover({ position: { x: 15, y: 15 } });
    const cover = page.locator('.cover').first();
    const t = await cover.evaluate((el) => getComputedStyle(el).transform);
    expect(t).not.toBe('none');
    await expect(cover).toHaveClass(/holo-on/);
    const sheen = await cover.evaluate((el) => getComputedStyle(el, '::after').opacity);
    expect(Number(sheen)).toBeGreaterThan(0);
  });
});
