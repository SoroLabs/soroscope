/**
 * Copy feedback state machine shared by every CopyButton instance.
 *
 * The logic lives here (instead of inside the React component) so it can be
 * unit tested with `node --test` without a DOM or a React renderer.
 */

const COPY_STATE = {
  IDLE: 'idle',
  COPIED: 'copied',
  ERROR: 'error',
};

const DEFAULT_FEEDBACK_TIMEOUT = 2000;

/**
 * Tracks the transient "Copied!" / "Copy failed" feedback window.
 *
 * Every trigger restarts the window, so repeated clicks keep the badge visible
 * until the user stops clicking. `dispose()` must be called on unmount.
 */
function createCopyFeedback(options = {}) {
  const {
    timeout = DEFAULT_FEEDBACK_TIMEOUT,
    setTimeoutFn = setTimeout,
    clearTimeoutFn = clearTimeout,
    onChange = () => {},
  } = options;

  const duration = Number.isFinite(timeout) && timeout > 0 ? timeout : DEFAULT_FEEDBACK_TIMEOUT;

  let state = COPY_STATE.IDLE;
  let timerId = null;

  const clearTimer = () => {
    if (timerId !== null) {
      clearTimeoutFn(timerId);
      timerId = null;
    }
  };

  const schedule = () => {
    clearTimer();
    timerId = setTimeoutFn(() => {
      timerId = null;
      setState(COPY_STATE.IDLE);
    }, duration);
  };

  const setState = (nextState) => {
    if (state === nextState) {
      return state;
    }
    state = nextState;
    onChange(state);
    return state;
  };

  return {
    get state() {
      return state;
    },
    get isCopied() {
      return state === COPY_STATE.COPIED;
    },
    get isError() {
      return state === COPY_STATE.ERROR;
    },
    get isIdle() {
      return state === COPY_STATE.IDLE;
    },
    get timeout() {
      return duration;
    },
    succeed() {
      setState(COPY_STATE.COPIED);
      schedule();
      return state;
    },
    fail() {
      setState(COPY_STATE.ERROR);
      schedule();
      return state;
    },
    reset() {
      clearTimer();
      return setState(COPY_STATE.IDLE);
    },
    dispose() {
      clearTimer();
    },
  };
}

/**
 * Writes text to the clipboard, falling back to a hidden textarea when the
 * async Clipboard API is unavailable (non-secure contexts, older browsers).
 *
 * Rejects with a descriptive Error so callers can surface a failure state.
 */
async function writeTextToClipboard(text, deps = {}) {
  const {
    clipboard = typeof navigator !== 'undefined' ? navigator.clipboard : undefined,
    doc = typeof document !== 'undefined' ? document : undefined,
  } = deps;

  const value = typeof text === 'string' ? text : String(text ?? '');

  if (value.length === 0) {
    throw new Error('Nothing to copy');
  }

  if (clipboard && typeof clipboard.writeText === 'function') {
    await clipboard.writeText(value);
    return value;
  }

  if (!doc || typeof doc.execCommand !== 'function') {
    throw new Error('Clipboard API is unavailable in this browser');
  }

  const textArea = doc.createElement('textarea');
  textArea.value = value;
  textArea.setAttribute('readonly', '');
  textArea.setAttribute('aria-hidden', 'true');
  textArea.style.position = 'fixed';
  textArea.style.top = '0';
  textArea.style.left = '-9999px';
  textArea.style.opacity = '0';

  const previouslyFocused = doc.activeElement;
  doc.body.appendChild(textArea);

  try {
    textArea.select();
    if (textArea.setSelectionRange) {
      textArea.setSelectionRange(0, value.length);
    }

    const copied = doc.execCommand('copy');
    if (copied === false) {
      throw new Error('Clipboard write was rejected by the browser');
    }

    return value;
  } finally {
    doc.body.removeChild(textArea);
    if (previouslyFocused && typeof previouslyFocused.focus === 'function') {
      previouslyFocused.focus();
    }
  }
}

/**
 * Maps a copy failure to a short, user facing message. Falls back to a generic
 * message so the UI never renders a raw error string.
 */
function describeCopyError(error) {
  const fallback = 'Copy failed. Try again.';

  if (!error) {
    return fallback;
  }

  if (typeof error === 'string') {
    return error || fallback;
  }

  const message = typeof error.message === 'string' ? error.message.trim() : '';
  if (!message) {
    return fallback;
  }

  if (error.name === 'NotAllowedError' || /permission|denied|not allowed/i.test(message)) {
    return 'Copy blocked by the browser. Allow clipboard access and try again.';
  }

  if (error.name === 'TypeError' && /clipboard|undefined|not a function/i.test(message)) {
    return 'Copy is not supported in this browser.';
  }

  return message;
}

module.exports = {
  COPY_STATE,
  DEFAULT_FEEDBACK_TIMEOUT,
  createCopyFeedback,
  writeTextToClipboard,
  describeCopyError,
};
