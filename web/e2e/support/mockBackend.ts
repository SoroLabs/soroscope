import type { Page, Request } from '@playwright/test';

/** Smallest valid WebAssembly module: magic number + version 1. */
export const MINIMAL_WASM = Buffer.from('0061736d01000000', 'hex');

/** A realistic `/analyze` response with non-zero CPU and RAM usage. */
export function mockReport(overrides: Partial<Record<string, unknown>> = {}) {
  return {
    cpu_instructions: 1_234_567,
    ram_bytes: 456_789,
    ledger_read_bytes: 2_048,
    ledger_write_bytes: 1_024,
    transaction_size_bytes: 512,
    cost_stroops: 15_000,
    bytes_by_durability: {
      read: { code: 0, instance: 1_024, persistent: 1_024, temporary: 0, other: 0 },
      write: { code: 0, instance: 512, persistent: 512, temporary: 0, other: 0 },
    },
    state_dependency: null,
    ttl_analysis: null,
    entry_size_analysis: { max_entry_size_bytes: 65_536, entries: [], unmeasured: 0 },
    nutrition: { efficiency_score: 92, insights: [] },
    call_graph: null,
    call_graph_mermaid: null,
    state_snapshot: null,
    protocol_version: 22,
    ...overrides,
  };
}

export interface MockBackend {
  /** Bodies of every `/analyze` (contract) request, in order. */
  analyzeRequests: Array<Record<string, unknown>>;
  /** Bodies of every `/analyze/wasm` request, in order. */
  wasmRequests: Array<Record<string, unknown>>;
}

function jsonBody(request: Request): Record<string, unknown> {
  try {
    return (request.postDataJSON() as Record<string, unknown>) ?? {};
  } catch {
    return {};
  }
}

/**
 * Stubs the Rust simulation engine so E2E tests run without a backend.
 * Every analyze endpoint answers with `mockReport()`.
 */
export async function mockBackend(page: Page): Promise<MockBackend> {
  const backend: MockBackend = { analyzeRequests: [], wasmRequests: [] };
  const json = (body: unknown) => ({
    status: 200,
    contentType: 'application/json',
    body: JSON.stringify(body),
  });

  await page.route(/\/analyze\/wasm(\?|$)/, async (route) => {
    backend.wasmRequests.push(jsonBody(route.request()));
    await route.fulfill(json(mockReport()));
  });
  await page.route(/\/analyze\/gas-golfing(\?|$)/, async (route) => {
    await route.fulfill(json({ report: { suggestions: [] } }));
  });
  await page.route(/\/analyze(\?|$)/, async (route) => {
    backend.analyzeRequests.push(jsonBody(route.request()));
    await route.fulfill(json(mockReport()));
  });

  return backend;
}
