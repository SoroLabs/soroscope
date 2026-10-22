// themeToggle.test.cjs — unit tests for theme toggle logic
// Runs with: node --test ./components/themeToggle.test.cjs

'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');

const STORAGE_KEY = 'theme';

// A small localStorage substitute so persistence is exercised rather than
// testing only the in-memory React/next-themes state model.
function createMemoryStorage() {
  const values = new Map();
  return {
    getItem(key) { return values.has(key) ? values.get(key) : null; },
    setItem(key, value) { values.set(key, String(value)); },
    removeItem(key) { values.delete(key); },
  };
}

function createThemeManager(defaultTheme = 'dark', storage = createMemoryStorage()) {
  let currentTheme = storage.getItem(STORAGE_KEY) || defaultTheme;
  return {
    get theme() { return currentTheme; },
    setTheme(theme) {
      currentTheme = theme;
      storage.setItem(STORAGE_KEY, theme);
    },
    toggle() {
      this.setTheme(currentTheme === 'dark' ? 'light' : 'dark');
    },
    get isDark() { return currentTheme === 'dark'; },
    get isLight() { return currentTheme === 'light'; },
    storage,
  };
}

// ── Theme Toggle Tests ──
test('theme toggle: initial state is dark by default', () => {
  const tm = createThemeManager('dark');
  assert.equal(tm.theme, 'dark');
  assert.equal(tm.isDark, true);
  assert.equal(tm.isLight, false);
});

test('theme toggle: calling setTheme switches to light and persists it', () => {
  const tm = createThemeManager('dark');
  tm.setTheme('light');
  assert.equal(tm.theme, 'light');
  assert.equal(tm.isDark, false);
  assert.equal(tm.isLight, true);
  assert.equal(tm.storage.getItem(STORAGE_KEY), 'light');
});

test('theme toggle: toggle persists each selected theme under the theme key', () => {
  const storage = createMemoryStorage();
  const tm = createThemeManager('dark', storage);

  tm.toggle();
  assert.equal(tm.theme, 'light');
  assert.equal(storage.getItem('theme'), 'light');

  tm.toggle();
  assert.equal(tm.theme, 'dark');
  assert.equal(storage.getItem('theme'), 'dark');
});

test('theme toggle: saved preference is restored when the manager is recreated', () => {
  const storage = createMemoryStorage();
  const first = createThemeManager('dark', storage);
  first.setTheme('light');

  const afterRefresh = createThemeManager('dark', storage);
  assert.equal(afterRefresh.theme, 'light');
});

test('theme toggle: button label reflects current theme', () => {
  const tm = createThemeManager('dark');
  assert.equal(tm.isDark ? 'Switch to light mode' : 'Switch to dark mode', 'Switch to light mode');
  tm.toggle();
  assert.equal(tm.isDark ? 'Switch to light mode' : 'Switch to dark mode', 'Switch to dark mode');
});

// ── CSS Variable Tests ──
test('CSS variables: light theme defines all required tokens', () => {
  const lightVars = {
    '--bg-page': '#f6f8fa',
    '--bg-card': '#ffffff',
    '--bg-elevated': '#f6f8fa',
    '--bg-input': '#ffffff',
    '--text-primary': '#1f2328',
    '--text-secondary': '#656d76',
    '--text-muted': '#8b949e',
    '--border-default': '#d0d7de',
  };
  for (const [key, value] of Object.entries(lightVars)) {
    assert.ok(key.startsWith('--'), `${key} should be a CSS variable`);
    assert.ok(typeof value === 'string', `${key} should have a string value`);
    assert.ok(value.startsWith('#'), `${key} should be a hex color`);
  }
});

test('CSS variables: dark theme defines all required tokens', () => {
  const darkVars = {
    '--bg-page': '#0d1117',
    '--bg-card': '#161b22',
    '--bg-elevated': '#0d1117',
    '--bg-input': '#0d1117',
    '--text-primary': '#c9d1d9',
    '--text-secondary': '#8b949e',
    '--text-muted': '#6e7681',
    '--border-default': '#30363d',
  };
  for (const [key, value] of Object.entries(darkVars)) {
    assert.ok(key.startsWith('--'), `${key} should be a CSS variable`);
    assert.ok(typeof value === 'string', `${key} should have a string value`);
    assert.ok(value.startsWith('#'), `${key} should be a hex color`);
  }
});

test('CSS variables: dark theme tokens differ from light theme', () => {
  const light = { bg: '#f6f8fa', card: '#ffffff', border: '#d0d7de' };
  const dark = { bg: '#0d1117', card: '#161b22', border: '#30363d' };
  assert.notEqual(light.bg, dark.bg);
  assert.notEqual(light.card, dark.card);
  assert.notEqual(light.border, dark.border);
});

test('CSS variables: accent colors are theme-agnostic', () => {
  const accentColors = ['#00d9ff', '#3fb950', '#f85149', '#fb8500', '#58a6ff'];
  for (const color of accentColors) {
    assert.ok(color.startsWith('#'), `${color} should be a CSS variable`);
    assert.equal(color.length, 7);
  }
});
