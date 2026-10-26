// The small line icons used across the window.

const svg = (body, w = 16) =>
  `<svg class="ico" width="${w}" height="${w}" viewBox="0 0 16 16" fill="none" aria-hidden="true">${body}</svg>`;
export const ICONS = {
  grid: svg(
    '<rect x="1.7" y="1.7" width="5.1" height="5.1" rx="1.3" fill="currentColor"/><rect x="9.2" y="1.7" width="5.1" height="5.1" rx="1.3" fill="currentColor"/><rect x="1.7" y="9.2" width="5.1" height="5.1" rx="1.3" fill="currentColor"/><rect x="9.2" y="9.2" width="5.1" height="5.1" rx="1.3" fill="currentColor"/>',
  ),
  doc: svg(
    '<rect x="3.1" y="1.6" width="9.8" height="12.8" rx="2" stroke="currentColor" stroke-width="1.4"/><path d="M5.4 5.3h5.2M5.4 8h5.2M5.4 10.7h3.4" stroke="currentColor" stroke-width="1.2" stroke-linecap="round"/>',
  ),
  sliders: svg(
    '<path d="M2 4.4h12M2 11.6h12" stroke="currentColor" stroke-width="1.4" stroke-linecap="round"/><circle cx="5.8" cy="4.4" r="1.9" fill="currentColor"/><circle cx="10.2" cy="11.6" r="1.9" fill="currentColor"/>',
  ),
  play: svg('<path d="M4.4 2.6v10.8L13 8z" fill="currentColor"/>', 12),
  plus: svg('<path d="M8 2.4v11.2M2.4 8h11.2" stroke="currentColor" stroke-width="1.7" stroke-linecap="round"/>', 14),
  check: svg(
    '<path d="M3.2 8.4l3 3 6.6-6.8" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/>',
    14,
  ),
  back: svg(
    '<path d="M9.8 3.4L5.2 8l4.6 4.6" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>',
    14,
  ),
  chevron: svg(
    '<path d="M4.4 6.2L8 9.8l3.6-3.6" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>',
    12,
  ),
  x: svg('<path d="M4 4l8 8M12 4l-8 8" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>', 14),
  stop: svg('<rect x="4.2" y="4.2" width="7.6" height="7.6" rx="1.5" fill="currentColor"/>', 12),
  folder: svg(
    '<path d="M2 4.4c0-.7.6-1.2 1.2-1.2h2.4l1.3 1.5h5.9c.7 0 1.2.5 1.2 1.2v5.7c0 .7-.5 1.2-1.2 1.2H3.2c-.6 0-1.2-.5-1.2-1.2V4.4z" stroke="currentColor" stroke-width="1.3"/>',
  ),
  key: svg(
    '<circle cx="5.6" cy="5.6" r="2.9" stroke="currentColor" stroke-width="1.4"/><path d="M7.7 7.7l4.6 4.6M10.2 10.2l1.5-1.5" stroke="currentColor" stroke-width="1.4" stroke-linecap="round"/>',
  ),
  cpu: svg(
    '<rect x="3.4" y="3.4" width="9.2" height="9.2" rx="1.6" stroke="currentColor" stroke-width="1.4"/><rect x="6.4" y="6.4" width="3.2" height="3.2" rx=".8" fill="currentColor"/>',
  ),
  spark: svg('<path d="M8 1.8l1.4 4.8L14.2 8l-4.8 1.4L8 14.2 6.6 9.4 1.8 8l4.8-1.4z" fill="currentColor"/>', 14),
  screen: svg(
    '<rect x="2.2" y="3.2" width="11.6" height="8" rx="1.4" stroke="currentColor" stroke-width="1.4"/><path d="M5.4 13.4h5.2" stroke="currentColor" stroke-width="1.4" stroke-linecap="round"/>',
    14,
  ),
  bug: svg(
    '<ellipse cx="8" cy="8.8" rx="3.3" ry="4.2" stroke="currentColor" stroke-width="1.3"/><path d="M8 4.6V2.9M5.3 6L3.7 4.4M10.7 6l1.6-1.6M4.7 8.8H2.4M13.6 8.8h-2.3M5.4 11.4l-1.7 1.5M10.6 11.4l1.7 1.5" stroke="currentColor" stroke-width="1.2" stroke-linecap="round"/>',
    14,
  ),
  pad: svg(
    '<path d="M4.5 5.3h7c1.3 0 2.2.9 2.5 2.2l.6 2.6c.3 1.3-.8 2.3-2 1.9l-1.5-.6c-.5-.2-1-.6-1.6-.6H6.5c-.6 0-1 .4-1.6.6l-1.5.6c-1.2.4-2.3-.6-2-1.9l.6-2.6c.3-1.3 1.2-2.2 2.5-2.2z" stroke="currentColor" stroke-width="1.25"/><path d="M5.1 8.2v2.1M4.1 9.25h2.1" stroke="currentColor" stroke-width="1.25" stroke-linecap="round"/><circle cx="10.8" cy="8.7" r=".8" fill="currentColor"/><circle cx="11.9" cy="10" r=".8" fill="currentColor"/>',
    16,
  ),
};
