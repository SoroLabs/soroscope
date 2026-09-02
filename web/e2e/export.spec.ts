import { test, expect } from '@playwright/test';

test('Export report as file', async ({ page }) => {
  await page.goto('/');

  // Upload and simulate to get to export step
  await page.setInputFiles('input[type="file"]', {
    name: 'module.wasm',
    mimeType: 'application/wasm',
    buffer: Buffer.from('0061736d01000000', 'hex'),
  });
  await page.getByRole('button', { name: /simulate|run/i }).click();

  // Click export button and wait for download
  const downloadPromise = page.waitEvent('download');
  await page.getByRole('button', { name: /export|download/i }).click();
  const download = await downloadPromise;
  expect(download.suggestedFilename()).contain('report');
});