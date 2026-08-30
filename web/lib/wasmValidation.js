/**
 * App-facing entry point for the WASM decoder.
 *
 * The implementation lives in `public/wasm/wasmValidation.js` so the Web Worker
 * can `importScripts()` it directly; this re-export keeps normal
 * `from '../lib/wasmValidation'` imports (and the unit tests) unchanged.
 */

module.exports = require('../public/wasm/wasmValidation.js');
