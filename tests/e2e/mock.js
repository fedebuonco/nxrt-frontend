/**
 * Stand in for Tauri in a plain browser. It answers the commands the window uses on boot and records every call;
 * pad events can be pushed with `window.__emit` so the controller map can be exercised without hardware.
 */
export async function installMock(page, { snapshot, update }) {
  await page.addInitScript(
    ({ snapshot, update }) => {
      const calls = [];
      const listeners = {};
      window.__calls = calls;
      window.__TAURI__ = {
        core: {
          invoke: async (cmd, args) => {
            calls.push({ cmd, args });
            if (cmd === 'snapshot_command') return structuredClone(snapshot);
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
    { snapshot, update },
  );
}
