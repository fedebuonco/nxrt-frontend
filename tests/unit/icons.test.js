import { describe, expect, it } from 'vitest';
import { ICONS } from '../../src/js/icons.js';

describe('ICONS', () => {
  it('has every icon the window asks for', () => {
    expect(Object.keys(ICONS).sort()).toEqual([
      'back',
      'bug',
      'check',
      'chevron',
      'cpu',
      'doc',
      'folder',
      'grid',
      'key',
      'pad',
      'play',
      'plus',
      'screen',
      'sliders',
      'spark',
      'stop',
      'x',
    ]);
  });

  it('renders a decorative 16px SVG for each one', () => {
    for (const [name, icon] of Object.entries(ICONS)) {
      expect(icon, name).toContain('<svg');
      expect(icon, name).toContain('viewBox="0 0 16 16"');
      expect(icon, name).toContain('aria-hidden="true"');
      expect(icon, name).toContain('class="ico"');
    }
  });
});
