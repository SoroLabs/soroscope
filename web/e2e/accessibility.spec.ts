import { test, expect } from '@playwright/test';

const dashboardRoutes = ['/', '/settings'];

for (const route of dashboardRoutes) {
  test(`has no axe accessibility violations on ${route}`, async ({ page }) => {
    await page.goto(route);
    await page.addScriptTag({ path: require.resolve('axe-core/axe.min.js') });

    const results = await page.evaluate(async () => {
      const axe = (window as Window & { axe: { run: (context: Document, options: object) => Promise<{ violations: Array<{ id: string; impact: string; help: string; nodes: Array<{ target: string[] }> }> }> } }).axe;
      return axe.run(document, {
        runOnly: {
          type: 'tag',
          values: ['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa', 'best-practice'],
        },
      });
    });

    expect(results.violations, JSON.stringify(results.violations, null, 2)).toEqual([]);
  });
}
