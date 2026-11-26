/**
 * Client-side input sanitization helpers (issue #155).
 *
 * React escapes text nodes by default, but a few sinks accept raw HTML or
 * pass attacker-controllable strings into URL/JSON builders. These helpers
 * provide defense-in-depth: length caps, control-character stripping, HTML
 * escaping for innerHTML sinks, and shape checks for Soroban identifiers.
 */

/** Hard cap for generic text inputs. */
const DEFAULT_MAX_LENGTH = 4096;

/** Hard cap for a mermaid definition rendered into the DOM (200 KB). */
const MAX_MERMAID_LENGTH = 200000;

/** C0 plus DEL control characters (never valid in user-facing text). */
const CONTROL_CHAR_PATTERN = /[\u0000-\u0008\u000b\u000c\u000e-\u001f\u007f]/g;

/**
 * Escape a string for safe interpolation into an HTML context (e.g. when a
 * value must be written via innerHTML). Prefer DOM text nodes or React text
 * children whenever possible; this is a fallback for raw-HTML sinks.
 *
 * @param {unknown} value
 * @returns {string}
 */
function escapeHtml(value) {
  if (typeof value !== 'string') return '';
  return value.replace(/[&<>"']/g, (char) => {
    switch (char) {
      case '&':
        return '&amp;';
      case '<':
        return '&lt;';
      case '>':
        return '&gt;';
      case '"':
        return '&quot;';
      default:
        return '&#39;';
    }
  });
}

/**
 * Remove control characters from a string. These are never valid in function
 * names, contract IDs, or display text and are a common smuggling vector.
 *
 * @param {unknown} value
 * @returns {string}
 */
function stripControlChars(value) {
  if (typeof value !== 'string') return '';
  return value.replace(CONTROL_CHAR_PATTERN, '');
}

/**
 * Sanitize a free-text field for safe display/transport: strips control
 * characters and caps the length.
 *
 * @param {unknown} value
 * @param {number} [maxLength]
 * @returns {string}
 */
function sanitizePlainText(value, maxLength = DEFAULT_MAX_LENGTH) {
  const text = stripControlChars(value).trim();
  if (text.length <= maxLength) return text;
  return text.slice(0, maxLength);
}

/**
 * Sanitize a mermaid definition before it is rendered into the DOM. Mermaid
 * graphs are produced by the analysis backend or restored from localStorage,
 * both untrusted, so we bound the size and strip control characters. Rendering
 * itself runs with mermaid's strict security level.
 *
 * @param {unknown} value
 * @returns {string}
 */
function sanitizeMermaidDefinition(value) {
  const text = stripControlChars(value).trim();
  if (text.length <= MAX_MERMAID_LENGTH) return text;
  return text.slice(0, MAX_MERMAID_LENGTH);
}

/**
 * Sanitize a user-typed Soroban contract ID before using it in a request or
 * rendering it. Returns '' when the value is missing/too long; otherwise keeps
 * only printable characters within a 64-char cap. Format validation happens
 * separately (isValidContractId) at submit time.
 *
 * @param {unknown} value
 * @returns {string}
 */
function sanitizeContractId(value) {
  const text = typeof value === 'string' ? value.trim() : '';
  if (text === '' || text.length > 64) return '';
  return stripControlChars(text);
}

/**
 * True when `value` looks like a 56-character Stellar/Soroban strkey
 * (G... / C... / A... followed by 55 base32 chars).
 *
 * @param {unknown} value
 * @returns {boolean}
 */
function isValidContractId(value) {
  return typeof value === 'string' && /^[GCA][A-Z2-7]{55}$/.test(value.trim());
}

module.exports = {
  DEFAULT_MAX_LENGTH,
  MAX_MERMAID_LENGTH,
  escapeHtml,
  stripControlChars,
  sanitizePlainText,
  sanitizeMermaidDefinition,
  sanitizeContractId,
  isValidContractId,
};