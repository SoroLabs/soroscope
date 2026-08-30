/**
 * Client-Side Request Queue Manager for Contract Simulation Calls.
 *
 * Prevents hitting Soroban RPC rate limits by queuing incoming simulation requests
 * and pacing execution to a configurable rate (default: max 2 requests per second).
 */

export interface RequestQueueOptions {
  /** Maximum number of requests allowed per second. Defaults to 2. */
  maxRequestsPerSecond?: number;
}

interface QueuedTask<T> {
  task: () => Promise<T>;
  resolve: (value: T | PromiseLike<T>) => void;
  reject: (reason?: any) => void;
}

export class RequestQueueManager {
  private queue: QueuedTask<any>[] = [];
  private maxRequestsPerSecond: number;
  private minIntervalMs: number;
  private lastExecutionTime: number = 0;
  private activeCount: number = 0;
  private timer: ReturnType<typeof setTimeout> | null = null;

  constructor(options: RequestQueueOptions = {}) {
    this.maxRequestsPerSecond = options.maxRequestsPerSecond ?? 2;
    this.minIntervalMs = Math.ceil(1000 / this.maxRequestsPerSecond);
  }

  /**
   * Enqueues an async task (e.g. RPC contract simulation call) to be executed within the rate limit.
   */
  public enqueue<T>(task: () => Promise<T>): Promise<T> {
    return new Promise<T>((resolve, reject) => {
      this.queue.push({ task, resolve, reject });
      this.processQueue();
    });
  }

  /**
   * Returns the current number of tasks waiting in the queue.
   */
  public getQueueLength(): number {
    return this.queue.length;
  }

  /**
   * Returns true if there are tasks currently executing or waiting in the queue.
   */
  public isProcessing(): boolean {
    return this.activeCount > 0 || this.queue.length > 0;
  }

  /**
   * Clears all pending tasks in the queue and rejects them with an error.
   */
  public clear(): void {
    if (this.timer !== null) {
      clearTimeout(this.timer);
      this.timer = null;
    }
    const cancelled = [...this.queue];
    this.queue = [];
    for (const item of cancelled) {
      item.reject(new Error('Request queue cleared'));
    }
  }

  private processQueue(): void {
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

  private executeNext(): void {
    if (this.queue.length === 0) {
      return;
    }

    const nextItem = this.queue.shift();
    if (!nextItem) return;

    this.activeCount++;
    this.lastExecutionTime = Date.now();

    try {
      const promise = nextItem.task();
      Promise.resolve(promise)
        .then(nextItem.resolve)
        .catch(nextItem.reject)
        .finally(() => {
          this.activeCount = Math.max(0, this.activeCount - 1);
          this.processQueue();
        });
    } catch (err) {
      nextItem.reject(err);
      this.activeCount = Math.max(0, this.activeCount - 1);
      this.processQueue();
    }
  }
}

/**
 * Singleton instance of Simulation Queue Manager configured for 2 requests per second limit.
 */
export const simulationQueueManager = new RequestQueueManager({ maxRequestsPerSecond: 2 });

/**
 * Higher-order function to wrap an async call with the rate-limiting simulation queue manager.
 */
export function wrapWithRateLimit<TArgs extends any[], TRet>(
  fn: (...args: TArgs) => Promise<TRet>,
  queueManager: RequestQueueManager = simulationQueueManager,
): (...args: TArgs) => Promise<TRet> {
  return (...args: TArgs) => queueManager.enqueue(() => fn(...args));
}
