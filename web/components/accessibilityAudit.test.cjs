// accessibilityAudit.test.cjs — unit tests verifying WCAG 2.1 AA accessibility compliance
// Runs with: node --test ./components/accessibilityAudit.test.cjs

'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');

// Helper functions for WCAG 2.1 contrast calculations
function hexToRgb(hex) {
  const clean = hex.replace('#', '');
  return [
    parseInt(clean.substring(0, 2), 16),
    parseInt(clean.substring(2, 4), 16),
    parseInt(clean.substring(4, 6), 16),
  ];
}

function relativeLuminance(r, g, b) {
  const [rs, gs, bs] = [r, g, b].map((c) => {
    const s = c / 255;
    return s <= 0.03928 ? s / 12.92 : Math.pow((s + 0.055) / 1.055, 2.4);
  });
  return 0.2126 * rs + 0.7152 * gs + 0.0722 * bs;
}

function contrastRatio(hex1, hex2) {
  const l1 = relativeLuminance(...hexToRgb(hex1));
  const l2 = relativeLuminance(...hexToRgb(hex2));
  const lighter = Math.max(l1, l2);
  const darker = Math.min(l1, l2);
  return (lighter + 0.05) / (darker + 0.05);
}

test('WCAG 2.1 AA Contrast: Light mode text tokens achieve >= 4.5:1 ratio against background', () => {
  const bgPage = '#f6f8fa';
  const bgCard = '#ffffff';

  const textPrimary = '#1f2328';
  const textSecondary = '#57606a';
  const textMuted = '#57606a';

  assert.ok(
    contrastRatio(textPrimary, bgPage) >= 4.5,
    `text-primary contrast ratio on page was ${contrastRatio(textPrimary, bgPage).toFixed(2)} (expected >= 4.5)`
  );
  assert.ok(
    contrastRatio(textSecondary, bgPage) >= 4.5,
    `text-secondary contrast ratio on page was ${contrastRatio(textSecondary, bgPage).toFixed(2)} (expected >= 4.5)`
  );
  assert.ok(
    contrastRatio(textMuted, bgPage) >= 4.5,
    `text-muted contrast ratio on page was ${contrastRatio(textMuted, bgPage).toFixed(2)} (expected >= 4.5)`
  );
  assert.ok(
    contrastRatio(textSecondary, bgCard) >= 4.5,
    `text-secondary contrast ratio on card was ${contrastRatio(textSecondary, bgCard).toFixed(2)} (expected >= 4.5)`
  );
});

test('WCAG 2.1 AA Contrast: Dark mode text tokens achieve >= 4.5:1 ratio against card and page', () => {
  const bgPage = '#0d1117';
  const bgCard = '#161b22';

  const textPrimary = '#c9d1d9';
  const textSecondary = '#9198a1';
  const textMuted = '#9198a1';

  assert.ok(
    contrastRatio(textPrimary, bgPage) >= 4.5,
    `dark text-primary contrast on page was ${contrastRatio(textPrimary, bgPage).toFixed(2)} (expected >= 4.5)`
  );
  assert.ok(
    contrastRatio(textSecondary, bgPage) >= 4.5,
    `dark text-secondary contrast on page was ${contrastRatio(textSecondary, bgPage).toFixed(2)} (expected >= 4.5)`
  );
  assert.ok(
    contrastRatio(textMuted, bgCard) >= 4.5,
    `dark text-muted contrast on card was ${contrastRatio(textMuted, bgCard).toFixed(2)} (expected >= 4.5)`
  );
});

test('globals.css token definitions match WCAG 2.1 AA requirements', () => {
  const cssPath = path.join(__dirname, '../styles/globals.css');
  const cssContent = fs.readFileSync(cssPath, 'utf8');

  // Verify light tokens
  assert.match(cssContent, /--text-primary:\s*#1f2328/);
  assert.match(cssContent, /--text-secondary:\s*#57606a/);
  assert.match(cssContent, /--text-muted:\s*#57606a/);

  // Verify dark tokens
  assert.match(cssContent, /--text-primary:\s*#c9d1d9/);
  assert.match(cssContent, /--text-secondary:\s*#9198a1/);
  assert.match(cssContent, /--text-muted:\s*#9198a1/);
});

test('tailwind.config.js overrides slate and gray for dark surface contrast', () => {
  const tailwindPath = path.join(__dirname, '../tailwind.config.js');
  const tailwindContent = fs.readFileSync(tailwindPath, 'utf8');

  assert.ok(
    tailwindContent.includes('slate[300]'),
    'tailwind.config.js should map slate text shades to lighter colors for WCAG contrast'
  );
  assert.ok(
    tailwindContent.includes('gray[300]'),
    'tailwind.config.js should map gray text shades to lighter colors for WCAG contrast'
  );
});

test('Interactive elements have accessible labels or roles', () => {
  const indexPagePath = path.join(__dirname, '../pages/index.tsx');
  const indexContent = fs.readFileSync(indexPagePath, 'utf8');

  // Clear analysis button has aria-label
  assert.ok(
    indexContent.includes('aria-label="Clear current analysis"'),
    'Clear analysis button should have an accessible label'
  );

  // Contract ID input has accessible label or aria-label
  assert.ok(
    indexContent.includes('aria-label="Soroban contract ID"'),
    'Contract ID input should have an aria-label'
  );

  // Source code viewer accordion has aria-expanded and aria-controls
  assert.ok(
    indexContent.includes('aria-expanded={expanded}'),
    'Source code viewer should have aria-expanded'
  );
  assert.ok(
    indexContent.includes('aria-controls="source-code-panel"'),
    'Source code viewer should have aria-controls'
  );
});

test('GasGolfingSuggestionsTable has accessible modal and table attributes', () => {
  const tablePath = path.join(__dirname, 'GasGolfingSuggestionsTable.tsx');
  const tableContent = fs.readFileSync(tablePath, 'utf8');

  assert.ok(
    tableContent.includes('role="dialog"'),
    'Modal preview should define role="dialog"'
  );
  assert.ok(
    tableContent.includes('aria-labelledby="gas-golfing-preview-title"'),
    'Modal preview should have aria-labelledby'
  );
  assert.ok(
    tableContent.includes('aria-label="Close suggestion preview"'),
    'Modal preview close button should have an aria-label'
  );
  assert.ok(
    tableContent.includes('scope="col"'),
    'Table headers should define scope="col"'
  );
});
