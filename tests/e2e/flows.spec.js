import { expect, test } from '@playwright/test';
import { snapshot, update } from './fixtures.js';
import { installMock } from './mock.js';

const demo = snapshot.games[0];
const calls = (page, cmd) => page.evaluate((c) => window.__calls.filter((x) => x.cmd === c), cmd);

async function open(page, answers = {}, snap = snapshot) {
  await installMock(page, { snapshot: snap, update, answers });
  await page.goto('/');
  await expect(page.getByRole('heading', { name: 'Library', exact: true })).toBeVisible();
}

async function openDemo(page) {
  await page.locator('[data-card] .title').first().click();
  await expect(page.getByRole('heading', { name: demo.title })).toBeVisible();
}

test('Play starts the game by its key', async ({ page }) => {
  await open(page);
  await openDemo(page);
  await page.locator('[data-act="play"], [data-act="next"]').first().click();
  await expect.poll(async () => (await calls(page, 'start_game')).length).toBe(1);
  const [c] = await calls(page, 'start_game');
  expect(c.args).toEqual({ key: demo.key, mode: 'play' });
});

test('a start that fails says why', async ({ page }) => {
  await open(page, { start_game: { __error: 'Echoes of the Crown is already running' } });
  await openDemo(page);
  await page.locator('[data-act="play"], [data-act="next"]').first().click();
  await expect(page.locator('.toast.error')).toContainText('already running');
});

test('a running game shows Stop, and Stop stops it by its key', async ({ page }) => {
  await open(page);
  await openDemo(page);
  await page.evaluate(
    (key) => window.__emit('run-state', { key, kind: 'game', playing: true, step: '', progress: null }),
    demo.key,
  );
  const running = structuredClone(snapshot);
  running.games[0].running = { key: demo.key, kind: 'game', playing: true, step: '', progress: null };
  running.running = 1;
  await page.evaluate((s) => (window.__answers.snapshot_command = s), running);
  await page.evaluate(() => window.__emit('library-changed', null));
  const stop = page.locator('[data-act="stop"]').first();
  await expect(stop).toBeVisible();
  await stop.click();
  await expect.poll(async () => (await calls(page, 'stop')).map((c) => c.args.key)).toEqual([demo.key]);
});

test('a finished run is told, and the library read again', async ({ page }) => {
  await open(page);
  const before = (await calls(page, 'snapshot_command')).length;
  await page.evaluate((key) => window.__emit('run-finished', { key, text: 'Stopped', ok: true }), demo.key);
  await expect(page.locator('.toast.ok')).toContainText('Stopped');
  await expect.poll(async () => (await calls(page, 'snapshot_command')).length).toBeGreaterThan(before);
  await page.evaluate(
    (key) => window.__emit('run-finished', { key, text: 'Last run: crashed (signal 11 SIGSEGV)', ok: false }),
    demo.key,
  );
  await expect(page.locator('.toast.error')).toContainText('SIGSEGV');
});

test('Remove removes the game by its key and goes back to the library', async ({ page }) => {
  const empty = { ...structuredClone(snapshot), games: [] };
  await open(page, { remove: empty });
  await openDemo(page);
  await page.locator('[data-act="remove"]').first().click();
  await expect.poll(async () => (await calls(page, 'remove')).map((c) => c.args)).toEqual([{ key: demo.key }]);
  await expect(page.getByRole('heading', { name: 'Library', exact: true })).toBeVisible();
  await expect(page.locator('.toast.ok')).toContainText('Removed');
});

test('adding a package opens the game it added', async ({ page }) => {
  const pkg = '/tmp/nxrt-test/games/second.nsp';
  const added = structuredClone(snapshot);
  added.games.push({
    ...structuredClone(demo),
    index: 1,
    key: 'tid:0100second000000',
    title: 'Second Light',
    title_id: '0100SECOND000000',
    package: pkg,
    exe: null,
    ready: false,
  });
  await open(page, { pick_package: pkg, add_package: added });
  await page.locator('[data-act="add-game"]').first().click();
  await expect.poll(async () => (await calls(page, 'add_package')).map((c) => c.args)).toEqual([{ path: pkg }]);
  await expect(page.getByRole('heading', { name: 'Second Light' })).toBeVisible();
  await expect(page.locator('.toast.ok')).toContainText('Added Second Light');
});

test('a package that cannot be read says why and stays in the library', async ({ page }) => {
  await open(page, { pick_package: '/x/bad.nsp', add_package: { __error: '/x/bad.nsp: failed program layout' } });
  await page.locator('[data-act="add-game"]').first().click();
  await expect(page.locator('.toast.error')).toContainText('failed program layout');
  await expect(page.getByRole('heading', { name: 'Library', exact: true })).toBeVisible();
});

test('the backend warning is shown once', async ({ page }) => {
  const warn = { ...structuredClone(snapshot), message: 'The library could not be read (bad); it was kept as x' };
  await open(page, {}, warn);
  await expect(page.locator('.toast.error')).toHaveCount(1);
  await expect(page.locator('.toast.error')).toContainText('kept as');
  await page.evaluate(() => window.__emit('library-changed', null));
  await expect.poll(async () => (await calls(page, 'snapshot_command')).length).toBeGreaterThan(1);
  await expect(page.locator('.toast.error')).toHaveCount(1);
});

test('a game removed elsewhere takes the window back to the library', async ({ page }) => {
  await open(page);
  await openDemo(page);
  await page.evaluate((s) => (window.__answers.snapshot_command = s), { ...structuredClone(snapshot), games: [] });
  await page.evaluate(() => window.__emit('library-changed', null));
  await expect(page.getByRole('heading', { name: 'Library', exact: true })).toBeVisible();
});

test('a library that cannot be read is said so', async ({ page }) => {
  await installMock(page, { snapshot, update, answers: { snapshot_command: { __error: 'disk on fire' } } });
  await page.goto('/');
  await expect(page.getByText('Could not read the library.')).toBeVisible();
  await expect(page.locator('.toast.error')).toContainText('disk on fire');
});

for (const [kind, want] of [
  ['files', { stage: 'files', src: demo.package }],
  ['generate', { stage: 'generate', src: null }],
  ['compile', { stage: 'compile', src: null }],
]) {
  test(`the next step (${kind}) starts that step`, async ({ page }) => {
    const snap = structuredClone(snapshot);
    snap.games[0].ready = false;
    snap.games[0].next = { label: kind, kind, primary: true };
    await open(page, {}, snap);
    await openDemo(page);
    await page.locator('[data-act="next"]').first().click();
    await expect
      .poll(async () => (await calls(page, 'start_step')).map((c) => c.args))
      .toEqual([{ key: demo.key, ...want }]);
  });
}

test('an nxnative game is built for Mac, then played', async ({ page }) => {
  const snap = structuredClone(snapshot);
  Object.assign(snap.games[0], {
    backend: 'nxnative',
    ready: false,
    state: 'Next: build it for this Mac',
    tone: 'warn',
    steps: { files: false, generated: false, code: false, builder: false, dev: true },
    next: { label: 'Build for Mac', kind: 'build', primary: true },
  });
  await open(page, {}, snap);
  await openDemo(page);
  await expect(page.locator('.steps .name').nth(1)).toHaveText('Build for Mac');
  await expect(page.locator('[data-act="start-screen"]')).toHaveCount(0);
  await page.locator('.steps [data-act="next"]').click();
  await expect.poll(async () => (await calls(page, 'start_dev')).map((c) => c.args)).toEqual([{ key: demo.key }]);
});

test('the backend switch changes what runs the game', async ({ page }) => {
  const switched = structuredClone(snapshot);
  switched.games[0].backend = 'nxnative';
  await open(page, { set_backend: switched });
  await openDemo(page);
  const picker = page.locator('select[data-act="backend"]');
  await expect(picker).toHaveValue('nxrt');
  await picker.selectOption('nxnative');
  await expect
    .poll(async () => (await calls(page, 'set_backend')).map((c) => c.args))
    .toEqual([{ key: demo.key, backend: 'nxnative' }]);
  await expect(picker).toHaveValue('nxnative');
  await expect(page.locator('.toast.ok')).toContainText('now runs with nxnative');
});

test('a backend switch that fails says why and keeps the old backend', async ({ page }) => {
  await open(page, { set_backend: { __error: 'nxnative needs the game NSP' } });
  await openDemo(page);
  const picker = page.locator('select[data-act="backend"]');
  await picker.selectOption('nxnative');
  await expect(page.locator('.toast.error')).toContainText('needs the game NSP');
  await expect(picker).toHaveValue('nxrt');
});

test('the backend switch is a dropdown with every offered backend', async ({ page }) => {
  await open(page);
  await openDemo(page);
  const picker = page.locator('select[data-act="backend"]');
  await expect(picker.locator('option')).toHaveText(['nxrt', 'nxnative']);
  await expect(picker).toHaveAttribute('aria-label', 'Backend');
});

test('the backend choice survives a window reload', async ({ page }) => {
  const switched = structuredClone(snapshot);
  switched.games[0].backend = 'nxnative';
  await open(page, { set_backend: switched });
  await openDemo(page);
  await page.locator('select[data-act="backend"]').selectOption('nxnative');
  await expect(page.locator('select[data-act="backend"]')).toHaveValue('nxnative');
  await page.reload();
  await expect(page.getByRole('heading', { name: 'Library', exact: true })).toBeVisible();
  await openDemo(page);
  await expect(page.locator('select[data-act="backend"]')).toHaveValue('nxnative');
});

test('the backend switch is disabled while the game runs', async ({ page }) => {
  const running = structuredClone(snapshot);
  running.games[0].running = { playing: true };
  await open(page, {}, running);
  await openDemo(page);
  await expect(page.locator('select[data-act="backend"]')).toBeDisabled();
});

test('a game with one backend has no switch', async ({ page }) => {
  const one = structuredClone(snapshot);
  one.games[0].backends = ['nxrt'];
  await open(page, {}, one);
  await openDemo(page);
  await expect(page.locator('select[data-act="backend"]')).toHaveCount(0);
});
