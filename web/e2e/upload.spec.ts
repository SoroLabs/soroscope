import { test, expect } from '@playwright/test';

test('WASM file upload', async ({ page }) => {
  await page.goto('/');

  // Create a fake WASM file in memory
  const wasmBuffer = Buffer.from('0061736d01000000', 'hex'); // minimal WASM magic
  await page.setInputFiles('input[type="file"]', {
    name: 'module.wasm',
    mimeType: 'application/wasm',
    buffer: wasmBuffer,
  });

  // Expect a success message or file name to appear
  await expect(page.locator('text=module.wasm').first()).toBeEvisible();
});