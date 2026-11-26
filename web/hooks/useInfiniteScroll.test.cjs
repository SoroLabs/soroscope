// useInfiniteScroll.test.cjs — unit tests for IntersectionObserver infinite scroll hook logic
// Runs with: node --test ./hooks/useInfiniteScroll.test.cjs

'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');

// Mock IntersectionObserver for Node testing environment
class MockIntersectionObserver {
  constructor(callback, options = {}) {
    this.callback = callback;
    this.options = options;
    this.observedElement = null;
  }

  observe(element) {
    this.observedElement = element;
  }

  unobserve() {
    this.observedElement = null;
  }

  disconnect() {
    this.observedElement = null;
  }

  // Helper method to trigger intersection event in unit test
  trigger(isIntersecting) {
    this.callback([{ isIntersecting, target: this.observedElement }]);
  }
}

// Controller function replicating hook state/observer behavior for unit verification
function createInfiniteScrollController({ onLoadMore, hasMore, isLoading = false, threshold = 0.1, rootMargin = '100px' }) {
  let activeObserver = null;
  let currentTarget = null;
  let triggerCount = 0;

  const setTarget = (element) => {
    currentTarget = element;
    if (activeObserver) {
      activeObserver.disconnect();
      activeObserver = null;
    }

    if (!element || !hasMore || isLoading) {
      return;
    }

    activeObserver = new MockIntersectionObserver((entries) => {
      const entry = entries[0];
      if (entry && entry.isIntersecting && hasMore && !isLoading) {
        triggerCount++;
        onLoadMore();
      }
    }, { threshold, rootMargin });

    activeObserver.observe(element);
  };

  return {
    setTarget,
    getObserver: () => activeObserver,
    getTriggerCount: () => triggerCount,
    cleanup: () => {
      if (activeObserver) {
        activeObserver.disconnect();
        activeObserver = null;
      }
    },
  };
}

test('useInfiniteScroll: observes element and triggers onLoadMore when intersecting', () => {
  let loaded = false;
  const controller = createInfiniteScrollController({
    onLoadMore: () => { loaded = true; },
    hasMore: true,
    isLoading: false,
  });

  const dummyElement = { id: 'sentinel' };
  controller.setTarget(dummyElement);

  const observer = controller.getObserver();
  assert.ok(observer, 'Observer should be initialized and attached');

  observer.trigger(true);
  assert.equal(loaded, true, 'onLoadMore should have been called when intersecting');
  assert.equal(controller.getTriggerCount(), 1);
});

test('useInfiniteScroll: does not observe or trigger if hasMore is false', () => {
  let loaded = false;
  const controller = createInfiniteScrollController({
    onLoadMore: () => { loaded = true; },
    hasMore: false,
    isLoading: false,
  });

  controller.setTarget({ id: 'sentinel' });
  const observer = controller.getObserver();

  assert.equal(observer, null, 'Observer should not attach when hasMore is false');
  assert.equal(loaded, false);
});

test('useInfiniteScroll: does not observe or trigger if isLoading is true', () => {
  let loaded = false;
  const controller = createInfiniteScrollController({
    onLoadMore: () => { loaded = true; },
    hasMore: true,
    isLoading: true,
  });

  controller.setTarget({ id: 'sentinel' });
  const observer = controller.getObserver();

  assert.equal(observer, null, 'Observer should not attach when isLoading is true');
  assert.equal(loaded, false);
});

test('useInfiniteScroll: cleans up and disconnects observer when target released', () => {
  const controller = createInfiniteScrollController({
    onLoadMore: () => {},
    hasMore: true,
    isLoading: false,
  });

  controller.setTarget({ id: 'sentinel' });
  assert.ok(controller.getObserver());

  controller.setTarget(null);
  assert.equal(controller.getObserver(), null, 'Observer should disconnect on null target');
});
