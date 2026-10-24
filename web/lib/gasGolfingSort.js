const SEVERITY_RANK = {
  high: 3,
  medium: 2,
  low: 1,
};

function severityRank(severity) {
  const normalized = String(severity || '').toLowerCase();
  return SEVERITY_RANK[normalized] ?? 0;
}

function numberOrMin(value) {
  if (value === null || value === undefined) return Number.NEGATIVE_INFINITY;
  const num = typeof value === 'number' ? value : Number(value);
  return Number.isFinite(num) ? num : Number.NEGATIVE_INFINITY;
}

/**
 * @param {Array<{title: string, severity: string, description?: string, gas_saved_estimate?: number|null}>} suggestions
 * @param {'severity'|'gas_saved_estimate'} sortKey
 * @param {'asc'|'desc'} direction
 */
function sortGasGolfingSuggestions(suggestions, sortKey, direction) {
  const multiplier = direction === 'asc' ? 1 : -1;

  return [...suggestions].sort((a, b) => {
    if (sortKey === 'severity') {
      const diff = severityRank(a.severity) - severityRank(b.severity);
      if (diff !== 0) return diff * multiplier;

      const gasDiff =
        numberOrMin(a.gas_saved_estimate) - numberOrMin(b.gas_saved_estimate);
      if (gasDiff !== 0) return gasDiff * -1;

      return a.title.localeCompare(b.title) || compareIds(a, b);
    }

    const diff =
      numberOrMin(a.gas_saved_estimate) - numberOrMin(b.gas_saved_estimate);
    if (diff !== 0) return diff * multiplier;

    const sevDiff = severityRank(a.severity) - severityRank(b.severity);
    if (sevDiff !== 0) return sevDiff * -1;

    return a.title.localeCompare(b.title) || compareIds(a, b);
  });
}

function compareIds(a, b) {
  return String(a.id ?? '').localeCompare(String(b.id ?? ''));
}

function humanizePatternType(patternType) {
  const text = String(patternType || 'suggestion').replace(/[_-]+/g, ' ').trim();
  return text.charAt(0).toUpperCase() + text.slice(1);
}

/**
 * Map raw `/analyze/gas-golfing` suggestions onto the table's row shape and
 * guarantee every row a unique, stable `id` to use as its React key (#1285).
 *
 * The server sends `id`; for payloads from older servers that lack it, one is
 * derived from the rule and location. Repeated ids get a `#n` suffix so two
 * rows can never share a key.
 *
 * @param {Array<Record<string, unknown>>} raw
 */
function normalizeGasGolfingSuggestions(raw) {
  if (!Array.isArray(raw)) return [];

  const seen = new Map();
  return raw
    .filter((item) => item && typeof item === 'object')
    .map((item) => {
      const patternType = item.pattern_type ? String(item.pattern_type) : undefined;
      const location = item.location ? String(item.location) : undefined;
      const title = item.title
        ? String(item.title)
        : humanizePatternType(patternType);

      const baseId =
        typeof item.id === 'string' && item.id.trim()
          ? item.id.trim()
          : `${patternType ?? title}@${location ?? 'module'}`;
      const count = (seen.get(baseId) ?? 0) + 1;
      seen.set(baseId, count);

      const recommendation = item.recommendation ?? item.suggested_fix;
      const gas = item.gas_saved_estimate;

      return {
        id: count === 1 ? baseId : `${baseId}#${count}`,
        title,
        severity: String(item.severity ?? 'info'),
        description: item.description ? String(item.description) : undefined,
        recommendation: recommendation ? String(recommendation) : undefined,
        code_example: item.code_example ? String(item.code_example) : undefined,
        location,
        pattern_type: patternType,
        gas_saved_estimate: typeof gas === 'number' ? gas : null,
      };
    });
}

module.exports = {
  normalizeGasGolfingSuggestions,
  sortGasGolfingSuggestions,
};

