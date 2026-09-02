import { test, expect } from '@playwright/test';

test('Run simulation after upload', async ({ page }) => {
  await page.goto('/');

  // Upload a WASM file first
  await page.setInputFiles('input[type="file"]', {
    name: 'module.wasm',
    mimeType: 'application/wasm',
    buffer: Buffer.from('0061736d01000000', 'hex'),
  });

  // Trigger simulation
  await page.getByRole('button', { name: /simulate|run/i }).click();

  // Expect a simulation result section to be visible
  await expect(page.locator('section', { hasText: 'Simulation Result' })).toBeVisible();
});