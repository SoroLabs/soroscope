import { test, expect } from '@playwright/test';
import { MINIMAL_WASM, mockBackend } from './support/mockBackend';

test('Export report as file', async ({ page }) => {
  await mockBackend(page);
  await page.goto('/');

  // Uploading a contract runs the simulation and shows the result.
  await page.setInputFiles('input[type="file"]', {
    name: 'module.wasm',
    mimeType: 'application/wasm',
    buffer: MINIMAL_WASM,
  });
  const exportButton = page.getByRole('button', { name: 'Export Report' });
  await expect(exportButton).toBeVisible();

  const downloadPromise = page.waitForEvent('download');
  await exportButton.click();
  const download = await downloadPromise;
  expect(download.suggestedFilename()).toContain('report');
});
