import { test, expect, type Page } from '@playwright/test';
import { MINIMAL_WASM, mockBackend } from './support/mockBackend';

// Default contract IDs from `context/NetworkContext.tsx`.
const TESTNET_CONTRACT_ID = 'CAEZJVJ4N7P7GRUVD5NG5LYYH23AQHJUKQEUHW54LR5PGQX3V7FXD7Q';
const MAINNET_CONTRACT_ID = 'CCW67TSBZVTZPT2PP5FE62EBY6GL3KCQSXYGLF73264P3EGBB3LCGN4P';

/** The header renders a desktop and a mobile switcher; use whichever is shown. */
function networkSwitcher(page: Page) {
  return page
    .getByRole('button', { name: /Current network: .* Click to switch network/ })
    .filter({ visible: true })
    .first();
}

/** Selects a contract function that takes no inputs, so Simulate can be clicked directly. */
async function selectNoArgFunction(page: Page) {
  await page.getByRole('button', { name: /^symbol/ }).first().click();
  await expect(page.getByRole('heading', { name: 'symbol', level: 2 })).toBeVisible();
}

test.describe('simulation happy path', () => {
  test('uploading a valid .wasm shows a Nutrition Label with non-zero CPU and RAM', async ({ page }) => {
    const backend = await mockBackend(page);
    await page.goto('/');

    await page.setInputFiles('input[type="file"]', {
      name: 'contract.wasm',
      mimeType: 'application/wasm',
      buffer: MINIMAL_WASM,
    });

    await expect(page.getByRole('heading', { name: 'Nutrition Facts' })).toBeVisible();
    expect(backend.wasmRequests.length).toBeGreaterThan(0);

    const cpuMeter = page.getByRole('meter', { name: /^CPU Instructions:/ });
    const ramMeter = page.getByRole('meter', { name: /^Memory \(RAM\):/ });
    await expect(cpuMeter).toBeVisible();
    await expect(ramMeter).toBeVisible();

    // Each metric row reads "<label> <value> <unit>". The value must be a
    // non-zero number (compact notation, e.g. "1.2M" or "456.8K").
    const nonZero = (label: string, unit: string) =>
      new RegExp(`^${label.replace(/[()]/g, '\\$&')}\\s+(?!0(\\.0+)?\\s)\\d[\\d.,]*[KMBT]?\\s+${unit}$`);
    await expect(page.getByText(/^CPU Instructions\s/)).toHaveText(nonZero('CPU Instructions', 'instr'));
    await expect(page.getByText(/^Memory \(RAM\)\s/)).toHaveText(nonZero('Memory (RAM)', 'bytes'));
  });

  test('switching network re-runs the simulation on the new network', async ({ page }) => {
    const backend = await mockBackend(page);
    await page.goto('/');

    await selectNoArgFunction(page);
    await page.getByRole('button', { name: 'Simulate', exact: true }).click();
    await expect(page.getByRole('heading', { name: 'Nutrition Facts' })).toBeVisible();
    expect(backend.analyzeRequests).toHaveLength(1);
    expect(backend.analyzeRequests[0]).toMatchObject({
      contract_id: TESTNET_CONTRACT_ID,
      function_name: 'symbol',
    });

    await networkSwitcher(page).click();
    await page.getByRole('option', { name: /Mainnet \(Public\)/ }).click();

    await expect(networkSwitcher(page)).toHaveAccessibleName(/Current network: Mainnet/);
    await expect.poll(() => backend.analyzeRequests.length).toBe(2);
    expect(backend.analyzeRequests[1]).toMatchObject({
      contract_id: MAINNET_CONTRACT_ID,
      function_name: 'symbol',
    });
    await expect(page.locator('#contract-id-input')).toHaveValue(MAINNET_CONTRACT_ID);
    await expect(page.getByRole('heading', { name: 'Nutrition Facts' })).toBeVisible();
  });

  test('copy button puts the contract address on the clipboard', async ({ page, context }) => {
    await context.grantPermissions(['clipboard-read', 'clipboard-write']);
    await page.goto('/');

    const contractInput = page.locator('#contract-id-input');
    await expect(contractInput).toHaveValue(TESTNET_CONTRACT_ID);

    const copyButton = page.getByRole('button', { name: 'Copy ID to clipboard' });
    await copyButton.click();
    await expect(copyButton).toHaveAttribute('data-state', 'copied');

    const clipboardText = await page.evaluate(() => navigator.clipboard.readText());
    expect(clipboardText).toBe(TESTNET_CONTRACT_ID);
  });
});
