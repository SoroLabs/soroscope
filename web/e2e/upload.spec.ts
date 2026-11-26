import { test, expect } from '@playwright/test';
import { MINIMAL_WASM, mockBackend } from './support/mockBackend';

test('WASM file upload', async ({ page }) => {
  await mockBackend(page);
  await page.goto('/');

  await page.setInputFiles('input[type="file"]', {
    name: 'module.wasm',
    mimeType: 'application/wasm',
    buffer: MINIMAL_WASM,
  });

  // The uploaded file becomes the active WASM for the explorer.
  await expect(page.getByText('module.wasm').first()).toBeVisible();
});
