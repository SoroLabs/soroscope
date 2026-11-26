// requestQueue.test.cjs — unit tests for RequestQueueManager rate throttling logic
// Runs with: node --test ./lib/requestQueue.test.cjs

'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');

// JavaScript implementation of RequestQueueManager for node unit testing
class RequestQueueManager {
  constructor(options = {}) {
    this.maxRequestsPerSecond = options.maxRequestsPerSecond ?? 2;
    this.minIntervalMs = Math.ceil(1000 / this.maxRequestsPerSecond);
    this.queue = [];
    this.lastExecutionTime = 0;
    this.activeCount = 0;
    this.timer = null;
    this.listeners = new Set();
  }

  enqueue(task) {
    return new Promise((resolve, reject) => {
      this.queue.push({ task, resolve, reject });
      this.notifyListeners();
      this.processQueue();
    });
  }

  getQueueLength() {
    return this.queue.length;
  }

  getStatus() {
    return {
      waiting: this.queue.length,
      active: this.activeCount,
      isProcessing: this.isProcessing(),
      maxRequestsPerSecond: this.maxRequestsPerSecond,
    };
  }

  subscribe(listener) {
    this.listeners.add(listener);
    listener(this.getStatus());
    return () => this.listeners.delete(listener);
  }

  isProcessing() {
    return this.activeCount > 0 || this.queue.length > 0;
  }

  clear() {
    if (this.timer !== null) {
      clearTimeout(this.timer);
      this.timer = null;
    }
    const cancelled = [...this.queue];
    this.queue = [];
    this.notifyListeners();
    for (const item of cancelled) {
      item.reject(new Error('Request queue cleared'));
    }
  }

  processQueue() {
    if (this.queue.length === 0 || this.timer !== null) {
      return;
    }

    const now = Date.now();
    const timeSinceLast = now - this.lastExecutionTime;
    const waitTime = Math.max(0, this.minIntervalMs - timeSinceLast);

    if (waitTime > 0) {
      this.timer = setTimeout(() => {
        this.timer = null;
        this.executeNext();
      }, waitTime);
    } else {
      this.executeNext();
    }
  }

  executeNext() {
    if (this.queue.length === 0) {
      return;
    }

    const nextItem = this.queue.shift();
    if (!nextItem) return;

    this.activeCount++;
    this.lastExecutionTime = Date.now();
    this.notifyListeners();

    try {
      const promise = nextItem.task();
      Promise.resolve(promise)
        .then(nextItem.resolve)
        .catch(nextItem.reject)
        .finally(() => {
          this.activeCount = Math.max(0, this.activeCount - 1);
          this.notifyListeners();
          this.processQueue();
        });
    } catch (err) {
      nextItem.reject(err);
      this.activeCount = Math.max(0, this.activeCount - 1);
      this.notifyListeners();
      this.processQueue();
    }
  }

  notifyListeners() {
    const status = this.getStatus();
    for (const listener of this.listeners) listener(status);
  }
}

function wrapWithRateLimit(fn, queueManager) {
  return (...args) => queueManager.enqueue(() => fn(...args));
}

test('RequestQueueManager: executes a single task successfully', async () => {
  const q = new RequestQueueManager({ maxRequestsPerSecond: 10 });
  const result = await q.enqueue(async () => 'simulation_ok');
  assert.equal(result, 'simulation_ok');
});

test('RequestQueueManager: executes multiple queued tasks sequentially in FIFO order', async () => {
  const q = new RequestQueueManager({ maxRequestsPerSecond: 20 });
  const results = [];

  const task1 = q.enqueue(async () => { results.push(1); return 1; });
  const task2 = q.enqueue(async () => { results.push(2); return 2; });
  const task3 = q.enqueue(async () => { results.push(3); return 3; });

  await Promise.all([task1, task2, task3]);
  assert.deepEqual(results, [1, 2, 3]);
});

test('RequestQueueManager: enforces rate limit throttling interval between tasks', async () => {
  // maxRequestsPerSecond = 10 -> minIntervalMs = 100ms
  const q = new RequestQueueManager({ maxRequestsPerSecond: 10 });
  const start = Date.now();

  const task1 = q.enqueue(async () => 1);
  const task2 = q.enqueue(async () => 2);
  const task3 = q.enqueue(async () => 3);

  await Promise.all([task1, task2, task3]);
  const elapsed = Date.now() - start;

  // 3 tasks at 10 req/s means 2 intervals of ~100ms = ~200ms
  assert.ok(elapsed >= 150, `Elapsed time should be at least 150ms for 3 rate-limited requests, got ${elapsed}ms`);
});

test('RequestQueueManager: handles rejected tasks without crashing queue pipeline', async () => {
  const q = new RequestQueueManager({ maxRequestsPerSecond: 50 });

  const task1 = q.enqueue(async () => { throw new Error('RPC error 503'); });
  const task2 = q.enqueue(async () => 'recovered_task');

  await assert.rejects(task1, { message: 'RPC error 503' });
  const result2 = await task2;
  assert.equal(result2, 'recovered_task');
});

test('RequestQueueManager: clear() rejects queued tasks with cancellation error', async () => {
  const q = new RequestQueueManager({ maxRequestsPerSecond: 1 }); // slow queue

  // Start first task to block processQueue timer
  const task1 = q.enqueue(() => new Promise((res) => setTimeout(() => res('slow'), 200)));
  const task2 = q.enqueue(async () => 'never_runs');

  assert.equal(q.getQueueLength(), 1);
  q.clear();
  assert.equal(q.getQueueLength(), 0);

  await assert.rejects(task2, { message: 'Request queue cleared' });
  await task1;
});

test('wrapWithRateLimit: wraps async function with queue throttling', async () => {
  const q = new RequestQueueManager({ maxRequestsPerSecond: 50 });
  const rawFn = async (x, y) => x + y;
  const wrapped = wrapWithRateLimit(rawFn, q);

  const res = await wrapped(10, 20);
  assert.equal(res, 30);
});


test('RequestQueueManager: publishes queue status updates for badges', async () => {
  const q = new RequestQueueManager({ maxRequestsPerSecond: 20 });
  const statuses = [];
  const unsubscribe = q.subscribe((status) => statuses.push(status));

  const task = q.enqueue(() => new Promise((resolve) => setTimeout(() => resolve('done'), 10)));

  assert.equal(statuses[0].waiting, 0);
  assert.equal(statuses[1].waiting, 1);
  await task;
  await new Promise((resolve) => setImmediate(resolve));
  unsubscribe();

  assert.ok(statuses.some((status) => status.active === 1));
  assert.ok(statuses.at(-1).isProcessing === false);
  assert.equal(statuses.at(-1).maxRequestsPerSecond, 20);
});
