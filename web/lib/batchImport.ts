/**
 * Client-side parser for batch contract call definition files.
 *
 * Accepts either:
 *  - CSV  (headers: contract_id, function_name, args)
 *  - JSON (array of { contract_id, function_name, args? })
 *
 * Returns a list of parsed call definitions or throws a descriptive error.
 */

export interface BatchCallDefinition {
  /** Soroban contract ID (C-prefixed Stellar address). */
  contract_id: string;
  /** Name of the exported function to invoke. */
  function_name: string;
  /**
   * Optional positional arguments as strings. For CSV files the `args` column
   * is treated as a JSON array string, e.g. `["arg1","arg2"]`.
   */
  args?: string[];
}

export interface BatchParseResult {
  calls: BatchCallDefinition[];
  /** Non-fatal warnings encountered during parsing (skipped rows, etc.). */
  warnings: string[];
}

const MAX_CALLS = 500;

// ---------------------------------------------------------------------------
// CSV parser
// ---------------------------------------------------------------------------

/**
 * Minimal RFC-4180 CSV parser that handles quoted fields with embedded commas
 * and newlines. Returns rows as string arrays.
 */
function parseCsvRows(text: string): string[][] {
  const rows: string[][] = [];
  let row: string[] = [];
  let field = '';
  let inQuotes = false;

  for (let i = 0; i < text.length; i++) {
    const ch = text[i];
    const next = text[i + 1];

    if (inQuotes) {
      if (ch === '"' && next === '"') {
        // Escaped double-quote inside a quoted field
        field += '"';
        i++;
      } else if (ch === '"') {
        inQuotes = false;
      } else {
        field += ch;
      }
    } else {
      if (ch === '"') {
        inQuotes = true;
      } else if (ch === ',') {
        row.push(field.trim());
        field = '';
      } else if (ch === '\r' && next === '\n') {
        row.push(field.trim());
        field = '';
        rows.push(row);
        row = [];
        i++;
      } else if (ch === '\n' || ch === '\r') {
        row.push(field.trim());
        field = '';
        rows.push(row);
        row = [];
      } else {
        field += ch;
      }
    }
  }

  // Flush final field/row
  if (field || row.length > 0) {
    row.push(field.trim());
    rows.push(row);
  }

  return rows;
}

function parseCsv(text: string): BatchParseResult {
  const warnings: string[] = [];
  const rawRows = parseCsvRows(text).filter((r) => r.some((c) => c !== ''));
  if (rawRows.length === 0) {
    throw new Error('CSV file is empty.');
  }

  // Normalise header names to lowercase with underscores
  const headers = rawRows[0].map((h) => h.toLowerCase().replace(/[\s-]/g, '_'));

  const contractIdx = headers.indexOf('contract_id');
  const functionIdx = headers.indexOf('function_name');
  const argsIdx = headers.indexOf('args');

  if (contractIdx === -1 || functionIdx === -1) {
    throw new Error(
      `CSV must include "contract_id" and "function_name" columns. ` +
        `Found columns: ${rawRows[0].join(', ')}`,
    );
  }

  const calls: BatchCallDefinition[] = [];

  for (let i = 1; i < rawRows.length; i++) {
    const row = rawRows[i];
    const contractId = row[contractIdx] ?? '';
    const functionName = row[functionIdx] ?? '';

    if (!contractId || !functionName) {
      warnings.push(`Row ${i + 1}: skipped — contract_id or function_name is empty.`);
      continue;
    }

    let args: string[] | undefined;
    if (argsIdx !== -1 && row[argsIdx]) {
      try {
        const parsed = JSON.parse(row[argsIdx]);
        if (Array.isArray(parsed)) {
          args = parsed.map(String);
        } else {
          warnings.push(`Row ${i + 1}: "args" column is not a JSON array — ignoring args.`);
        }
      } catch {
        warnings.push(`Row ${i + 1}: could not parse "args" as JSON — ignoring args.`);
      }
    }

    calls.push({ contract_id: contractId, function_name: functionName, args });

    if (calls.length >= MAX_CALLS) {
      warnings.push(`File has more than ${MAX_CALLS} calls. Only the first ${MAX_CALLS} will be processed.`);
      break;
    }
  }

  return { calls, warnings };
}

// ---------------------------------------------------------------------------
// JSON parser
// ---------------------------------------------------------------------------

function parseJson(text: string): BatchParseResult {
  const warnings: string[] = [];

  let parsed: unknown;
  try {
    parsed = JSON.parse(text);
  } catch {
    throw new Error('Invalid JSON: the file could not be parsed.');
  }

  if (!Array.isArray(parsed)) {
    throw new Error('JSON batch file must be an array of call definitions.');
  }

  const calls: BatchCallDefinition[] = [];

  for (let i = 0; i < parsed.length; i++) {
    const item = parsed[i];
    if (!item || typeof item !== 'object') {
      warnings.push(`Item ${i + 1}: skipped — not an object.`);
      continue;
    }

    const obj = item as Record<string, unknown>;
    const contractId = typeof obj.contract_id === 'string' ? obj.contract_id.trim() : '';
    const functionName = typeof obj.function_name === 'string' ? obj.function_name.trim() : '';

    if (!contractId || !functionName) {
      warnings.push(`Item ${i + 1}: skipped — contract_id or function_name is missing or empty.`);
      continue;
    }

    let args: string[] | undefined;
    if (obj.args !== undefined) {
      if (Array.isArray(obj.args)) {
        args = obj.args.map(String);
      } else {
        warnings.push(`Item ${i + 1}: "args" is not an array — ignoring args.`);
      }
    }

    calls.push({ contract_id: contractId, function_name: functionName, args });

    if (calls.length >= MAX_CALLS) {
      warnings.push(
        `Array has more than ${MAX_CALLS} items. Only the first ${MAX_CALLS} will be processed.`,
      );
      break;
    }
  }

  return { calls, warnings };
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/**
 * Parse the text content of a CSV or JSON file into a list of batch call
 * definitions.
 *
 * @param text    Raw text content of the uploaded file.
 * @param format  `"csv"` or `"json"`. Auto-detected when omitted.
 */
export function parseBatchFile(
  text: string,
  format?: 'csv' | 'json',
): BatchParseResult {
  const fmt = format ?? (text.trimStart().startsWith('[') || text.trimStart().startsWith('{') ? 'json' : 'csv');

  if (fmt === 'json') {
    return parseJson(text);
  }
  return parseCsv(text);
}
