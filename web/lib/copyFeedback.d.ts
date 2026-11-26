export declare const COPY_STATE: {
  readonly IDLE: 'idle';
  readonly COPIED: 'copied';
  readonly ERROR: 'error';
};

export declare const DEFAULT_FEEDBACK_TIMEOUT: number;

export type CopyFeedbackState = 'idle' | 'copied' | 'error';

export interface CopyFeedbackOptions {
  timeout?: number;
  setTimeoutFn?: (handler: () => void, timeout: number) => unknown;
  clearTimeoutFn?: (handle: unknown) => void;
  onChange?: (state: CopyFeedbackState) => void;
}

export interface CopyFeedback {
  readonly state: CopyFeedbackState;
  readonly isCopied: boolean;
  readonly isError: boolean;
  readonly isIdle: boolean;
  readonly timeout: number;
  succeed(): CopyFeedbackState;
  fail(): CopyFeedbackState;
  reset(): CopyFeedbackState;
  dispose(): void;
}

export interface ClipboardDeps {
  clipboard?: {
    writeText: (text: string) => Promise<void> | void;
  } | null;
  doc?: Document | null;
}

export function createCopyFeedback(options?: CopyFeedbackOptions): CopyFeedback;

export function writeTextToClipboard(
  text: string,
  deps?: ClipboardDeps,
): Promise<string>;

export function describeCopyError(error: unknown): string;
