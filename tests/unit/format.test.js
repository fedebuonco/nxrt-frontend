import { afterEach, describe, expect, it, vi } from 'vitest';
import { ago, baseName, bytes, dirName, errText, esc, initials, samePath } from '../../src/js/format.js';

describe('esc', () => {
  it('escapes the five HTML characters', () => {
    expect(esc('<a href="x">&\'</a>')).toBe('&lt;a href=&quot;x&quot;&gt;&amp;&#39;&lt;/a&gt;');
  });

  it('turns nullish values into an empty string', () => {
    expect(esc(null)).toBe('');
    expect(esc(undefined)).toBe('');
    expect(esc(0)).toBe('0');
  });
});

describe('paths', () => {
  it('takes the last segment', () => {
    expect(baseName('/a/b/c.txt')).toBe('c.txt');
    expect(baseName('/a/b/')).toBe('b');
    expect(baseName('')).toBe('');
  });

  it('takes the parent folder', () => {
    expect(dirName('/a/b/c.txt')).toBe('/a/b');
    expect(dirName('/c')).toBe('/');
    expect(dirName('c')).toBe('/');
  });

  it('compares paths literally', () => {
    expect(samePath('/a/b', '/a/b')).toBe(true);
    expect(samePath('/a/b', '/a/c')).toBe(false);
    expect(samePath(null, '')).toBe(true);
  });
});

describe('errText', () => {
  it('unwraps strings, errors and other values', () => {
    expect(errText('plain')).toBe('plain');
    expect(errText(new Error('boom'))).toBe('boom');
    expect(errText({ message: 'object message' })).toBe('object message');
    expect(errText(42)).toBe('42');
  });
});

describe('initials', () => {
  it('uses the significant words', () => {
    expect(initials('The Legend of Zelda')).toBe('TLZ');
    expect(initials('Echoes of the Crown')).toBe('ETC');
  });

  it('never returns an empty badge', () => {
    expect(initials('')).toBe('?');
    expect(initials('a')).toBe('?');
  });
});

describe('ago', () => {
  afterEach(() => vi.useRealTimers());

  it('formats the recent past', () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date('2026-01-01T12:00:00Z'));
    const now = Math.floor(Date.now() / 1000);
    expect(ago(now - 10)).toBe('just now');
    expect(ago(now - 90)).toBe('1 min ago');
    expect(ago(now - 3 * 3600)).toBe('3 h ago');
    expect(ago(now - 2 * 86400)).toBe('2 d ago');
  });

  it('clamps the future to just now', () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date('2026-01-01T12:00:00Z'));
    expect(ago(Math.floor(Date.now() / 1000) + 500)).toBe('just now');
  });
});

describe('bytes', () => {
  it('rounds to KB and one decimal MB', () => {
    expect(bytes(0)).toBe('1 KB');
    expect(bytes(1500)).toBe('2 KB');
    expect(bytes(1_000_000)).toBe('1.0 MB');
    expect(bytes(2_500_000)).toBe('2.5 MB');
  });
});
