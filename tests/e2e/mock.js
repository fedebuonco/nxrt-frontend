/**
 * Stand in for Tauri in a plain browser. It answers the commands the window uses on boot and records every call;
 * pad events can be pushed with `window.__emit` so the controller map can be exercised without hardware.
 * `answers` gives a test's own replies: { cmd: value }, or { cmd: { __error: 'text' } } for a command that fails.
 * `window.__answers` can be changed from a test too.
 */
export async function installMock(page, { snapshot, update, answers = {} }) {
  await page.addInitScript(
    ({ snapshot, update, answers }) => {
      const calls = [];
      const storeKey = 'nxrt-test-snapshot';
      const stored = (() => {
        try {
          return JSON.parse(localStorage.getItem(storeKey) || 'null');
        } catch {
          return null;
        }
      })();
      const current = stored || snapshot;
      window.__answers = answers;
      const listeners = {};
      window.__calls = calls;
      window.__TAURI__ = {
        core: {
          invoke: async (cmd, args) => {
            calls.push({ cmd, args });
            if (cmd in window.__answers) {
              const a = structuredClone(window.__answers[cmd]);
              if (a && a.__error) throw a.__error;
              if (cmd === 'set_backend' && a) localStorage.setItem(storeKey, JSON.stringify(a));
              return a;
            }
            if (cmd === 'snapshot_command') return structuredClone(current);
            if (cmd === 'pad_status') return { connected: true, name: 'Xbox Wireless Controller', error: null };
            if (cmd === 'check_update') return update;
            if (cmd === 'logs') return [];
            return null;
          },
        },
        event: {
          listen: async (name, cb) => {
            listeners[name] = cb;
            return () => {};
          },
        },
      };
      window.__emit = (name, payload) => {
        const cb = listeners[name];
        if (cb) cb({ payload });
      };
      Object.defineProperty(document, 'hasFocus', { value: () => true, configurable: true });
    },
    { snapshot, update, answers },
  );
}
