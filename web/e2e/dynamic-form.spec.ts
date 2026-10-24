import { test, expect } from '@playwright/test';

test('form inputs are cleared when a different contract function is selected', async ({ page }) => {
  await page.goto('/');
  await expect(page.getByRole('heading', { name: 'transfer', level: 2 })).toBeVisible();

  const amount = page.locator('#contract-input-amount');
  const to = page.locator('#contract-input-to');
  await to.fill('GBRPYHIL2CI3FNQ4BXLFMNDLFJUNPU2HY3ZMFSHONUCEOASW7QC7OX2H');
  await amount.fill('1000');

  // `mint` also has `to` and `amount` inputs, so stale values would show here.
  await page.getByRole('button', { name: /^mint/ }).first().click();
  await expect(page.getByRole('heading', { name: 'mint', level: 2 })).toBeVisible();
  await expect(to).toHaveValue('');
  await expect(amount).toHaveValue('');
});
