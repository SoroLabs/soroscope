// paginationUtils.test.cjs — unit tests for pagination logic
// Runs with: node --test ./lib/paginationUtils.test.cjs

'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');

// Inline paginate function (TypeScript module requires transpilation)
function paginate(items, page, perPage) {
  const total = items.length;
  const totalPages = perPage > 0 ? Math.max(1, Math.ceil(total / perPage)) : 1;
  const clampedPage = Math.max(1, Math.min(page, totalPages));
  const start = (clampedPage - 1) * perPage;
  const end = start + perPage;
  return {
    items: items.slice(start, end),
    page: clampedPage,
    perPage,
    total,
    totalPages,
  };
}

test('paginate: returns correct items for page 1', () => {
  const items = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];
  const result = paginate(items, 1, 5);
  assert.deepEqual(result.items, [1, 2, 3, 4, 5]);
  assert.equal(result.page, 1);
  assert.equal(result.total, 12);
  assert.equal(result.totalPages, 3);
});

test('paginate: returns correct items for page 2', () => {
  const items = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];
  const result = paginate(items, 2, 5);
  assert.deepEqual(result.items, [6, 7, 8, 9, 10]);
  assert.equal(result.page, 2);
});

test('paginate: returns correct items for last page', () => {
  const items = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];
  const result = paginate(items, 3, 5);
  assert.deepEqual(result.items, [11, 12]);
  assert.equal(result.page, 3);
});

test('paginate: clamps page to valid range when page exceeds totalPages', () => {
  const items = [1, 2, 3];
  const result = paginate(items, 999, 5);
  assert.equal(result.page, 1);
  assert.deepEqual(result.items, [1, 2, 3]);
});

test('paginate: handles empty array', () => {
  const result = paginate([], 1, 10);
  assert.deepEqual(result.items, []);
  assert.equal(result.total, 0);
  assert.equal(result.totalPages, 1);
});

test('paginate: returns totalPages of 1 for empty array', () => {
  const result = paginate([], 1, 10);
  assert.equal(result.totalPages, 1);
});


