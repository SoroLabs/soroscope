/**
 * Shareable simulation result URLs (#1286).
 *
 * A share link is `<base>/result/<simulation_id>`, where the id comes from
 * the server's simulation persistence. The page at `pages/result/[id].tsx`
 * fetches the stored result back by id.
 */

const DEFAULT_SHARE_BASE_URL = 'https://soroscope.app';

// UUIDs, ULIDs and numeric/slug ids; nothing that could escape the path.
const SIMULATION_ID_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_-]{0,127}$/;

function isValidSimulationId(id) {
  return typeof id === 'string' && SIMULATION_ID_PATTERN.test(id);
}

function resolveShareBaseUrl(override) {
  const base =
    (typeof override === 'string' && override.trim()) || DEFAULT_SHARE_BASE_URL;
  return base.trim().replace(/\/+$/, '');
}

/**
 * @param {string} simulationId
 * @param {string} [baseUrl]
 */
function buildShareUrl(simulationId, baseUrl) {
  if (!isValidSimulationId(simulationId)) {
    throw new Error(`Invalid simulation id: ${String(simulationId)}`);
  }
  return `${resolveShareBaseUrl(baseUrl)}/result/${simulationId}`;
}

function toTimestamp(value) {
  if (typeof value === 'number' && Number.isFinite(value)) {
    // Seconds since epoch are < 1e12; milliseconds are larger.
    return value < 1e12 ? value * 1000 : value;
  }
  if (typeof value === 'string' && value.trim()) {
    const asNumber = Number(value);
    if (Number.isFinite(asNumber)) return toTimestamp(asNumber);
    const parsed = Date.parse(value);
    if (Number.isFinite(parsed)) return parsed;
  }
  return null;
}

function isObject(value) {
  return value !== null && typeof value === 'object' && !Array.isArray(value);
}

function pickReport(record) {
  for (const key of ['report', 'resource_report', 'resources', 'result']) {
    if (isObject(record[key]) && 'cpu_instructions' in record[key]) {
      return record[key];
    }
  }
  return 'cpu_instructions' in record ? record : undefined;
}

/**
 * Turn a stored simulation record into the shape `ResultViewer` renders.
 * Accepts the report nested (`report`, `resource_report`, ...) or flattened
 * onto the record, so it works with either server response layout.
 *
 * @param {unknown} record
 * @param {string} requestedId
 */
function simulationRecordToResult(record, requestedId) {
  if (!isObject(record)) {
    throw new Error('Simulation not found or response was malformed');
  }

  const report = pickReport(record);
  const simulationId = String(record.simulation_id ?? record.id ?? requestedId);
  const error = typeof record.error === 'string' ? record.error : undefined;
  const success =
    typeof record.success === 'boolean' ? record.success : error === undefined;
  const args = Array.isArray(record.args) ? record.args : [];
  const inputs = Object.fromEntries(args.map((value, i) => [`arg${i}`, String(value)]));

  const result = {
    id: simulationId,
    simulationId,
    functionName: String(record.function_name ?? record.functionName ?? 'unknown'),
    inputs,
    timestamp:
      toTimestamp(record.created_at ?? record.timestamp ?? record.analysis_timestamp) ??
      Date.now(),
    success,
  };

  if (error !== undefined) result.error = error;
  if (report) {
    result.analysisReport = report;
    result.resourceCost = report;
    if (report.state_snapshot) result.stateSnapshot = report.state_snapshot;
    if (report.call_graph) result.callGraph = report.call_graph;
    if (report.call_graph_mermaid) result.callGraphMermaid = report.call_graph_mermaid;
  }

  const contractId = record.contract_id ?? record.contractId;
  return {
    result,
    contractId: typeof contractId === 'string' ? contractId : undefined,
  };
}

module.exports = {
  DEFAULT_SHARE_BASE_URL,
  buildShareUrl,
  isValidSimulationId,
  resolveShareBaseUrl,
  simulationRecordToResult,
};
