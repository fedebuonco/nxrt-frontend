// Text, paths, time and other pure helpers shared by the window.

export const esc = (v) =>
  String(v ?? '').replace(
    /[&<>"']/g,
    (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c],
  );

export const baseName = (p) =>
  String(p || '')
    .split('/')
    .filter(Boolean)
    .pop() || String(p || '');
export const dirName = (p) =>
  String(p || '')
    .split('/')
    .slice(0, -1)
    .join('/') || '/';
export const errText = (e) => (typeof e === 'string' ? e : e && e.message ? e.message : String(e));
export const samePath = (a, b) => String(a || '') === String(b || '');

export function initials(name) {
  const words = String(name || '')
    .split(/[^A-Za-z0-9]+/)
    .filter((w) => w.length > 2 || /^[A-Z]+$/.test(w));
  const s = words
    .map((w) => w[0])
    .slice(0, 3)
    .join('');
  return (s || '?').toUpperCase();
}

export function ago(secs) {
  const now = Math.floor(Date.now() / 1000);
  const d = Math.max(0, now - Number(secs || 0));
  if (d < 60) return 'just now';
  if (d < 3600) return `${Math.floor(d / 60)} min ago`;
  if (d < 86400) return `${Math.floor(d / 3600)} h ago`;
  return `${Math.floor(d / 86400)} d ago`;
}

export function bytes(n) {
  n = Number(n || 0);
  if (n >= 1e6) return `${(n / 1e6).toFixed(1)} MB`;
  return `${Math.max(1, Math.round(n / 1e3))} KB`;
}
