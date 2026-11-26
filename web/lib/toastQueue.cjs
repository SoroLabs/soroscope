'use strict';

const MAX_VISIBLE_TOASTS = 5;
const DEFAULT_TOAST_DURATION_MS = 4000;
const SWIPE_DISMISS_THRESHOLD_PX = 60;

function enqueueToast(queue, toast) {
  return [...queue, toast];
}

function dismissToast(queue, id) {
  return queue.filter((toast) => toast.id !== id);
}

function getVisibleToasts(queue) {
  return queue.slice(0, MAX_VISIBLE_TOASTS);
}

module.exports = {
  DEFAULT_TOAST_DURATION_MS,
  MAX_VISIBLE_TOASTS,
  SWIPE_DISMISS_THRESHOLD_PX,
  dismissToast,
  enqueueToast,
  getVisibleToasts,
};
