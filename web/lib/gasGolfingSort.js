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

      return a.title.localeCompare(b.title);
    }

    const diff =
      numberOrMin(a.gas_saved_estimate) - numberOrMin(b.gas_saved_estimate);
    if (diff !== 0) return diff * multiplier;

    const sevDiff = severityRank(a.severity) - severityRank(b.severity);
    if (sevDiff !== 0) return sevDiff * -1;

    return a.title.localeCompare(b.title);
  });
}

module.exports = {
  sortGasGolfingSuggestions,
};

