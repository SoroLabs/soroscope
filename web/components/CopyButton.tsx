import React, { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { AlertTriangle, Check, Copy } from "lucide-react";
import {
  COPY_STATE,
  createCopyFeedback,
  describeCopyError,
  writeTextToClipboard,
} from "../lib/copyFeedback";

export interface CopyButtonProps {
  text: string;
  label?: string;
  copiedLabel?: string;
  errorLabel?: string;
  showTooltip?: boolean;
  tooltipPosition?: "top" | "bottom" | "left" | "right";
  timeout?: number;
  className?: string;
  iconSize?: number;
  variant?: "default" | "outline" | "ghost" | "icon";
  onCopy?: () => void;
  onError?: (error: unknown, message: string) => void;
}

export function CopyButton({
  text,
  label,
  copiedLabel = "Copied!",
  errorLabel = "Copy failed",
  showTooltip = true,
  tooltipPosition = "top",
  timeout = 2000,
  className = "",
  iconSize = 16,
  variant = "ghost",
  onCopy,
  onError,
}: CopyButtonProps) {
  const [status, setStatus] = useState<string>(COPY_STATE.IDLE);
  const [errorMessage, setErrorMessage] = useState("");
  const attemptRef = useRef(0);

  const feedbackTimeout =
    Number.isFinite(timeout) && timeout > 0 ? timeout : 2000;

  const feedback = useMemo(
    () => createCopyFeedback({ timeout: feedbackTimeout, onChange: setStatus }),
    [feedbackTimeout],
  );

  useEffect(() => {
    return () => feedback.dispose();
  }, [feedback]);

  const handleCopy = useCallback(
    async (e: React.MouseEvent<HTMLButtonElement>) => {
      e.stopPropagation();

      const attempt = attemptRef.current + 1;
      attemptRef.current = attempt;

      try {
        await writeTextToClipboard(text);

        // A newer click already settled the feedback state.
        if (attemptRef.current !== attempt) return;

        setErrorMessage("");
        feedback.succeed();
        if (onCopy) onCopy();
      } catch (err) {
        if (attemptRef.current !== attempt) return;

        const message = describeCopyError(err);
        setErrorMessage(message);
        feedback.fail();
        if (onError) onError(err, message);

        console.error("Failed to copy text: ", err);
      }
    },
    [feedback, onCopy, onError, text],
  );

  const isCopied = status === COPY_STATE.COPIED;
  const isError = status === COPY_STATE.ERROR;
  const activeLabel = isCopied ? copiedLabel : isError ? errorLabel : label;

  // Base styling variants
  let variantStyles = "";
  switch (variant) {
    case "default":
      variantStyles =
        "bg-cyan-600 text-white hover:bg-cyan-500 border border-cyan-500/30";
      break;
    case "outline":
      variantStyles =
        "border border-slate-700 bg-slate-900/80 text-slate-300 hover:bg-slate-800 hover:text-white";
      break;
    case "icon":
      variantStyles =
        "p-1.5 text-slate-400 hover:bg-slate-800 hover:text-slate-200 rounded-md";
      break;
    case "ghost":
    default:
      variantStyles =
        "bg-slate-900/60 text-slate-300 border border-slate-800 hover:bg-slate-800 hover:text-white";
      break;
  }

  // Feedback state accents use ring utilities so they never conflict with the
  // background/border colours of the selected variant.
  const activeStateStyles = isError
    ? "ring-1 ring-inset ring-rose-500/40"
    : "ring-1 ring-inset ring-emerald-500/40";

  // Tooltip position classes. The arrow is a rotated square whose visible
  // border side faces away from the tooltip.
  let tooltipPositionClasses = "";
  let tooltipArrowClasses = "";
  switch (tooltipPosition) {
    case "bottom":
      tooltipPositionClasses = "top-full mt-2 left-1/2 -translate-x-1/2";
      tooltipArrowClasses =
        "-top-1 left-1/2 -translate-x-1/2 border-b-slate-900 border-l-0 border-r-0 border-t-0";
      break;
    case "left":
      tooltipPositionClasses = "right-full mr-2 top-1/2 -translate-y-1/2";
      tooltipArrowClasses =
        "-right-1 top-1/2 -translate-y-1/2 border-l-slate-900 border-b-0 border-t-0 border-r-0";
      break;
    case "right":
      tooltipPositionClasses = "left-full ml-2 top-1/2 -translate-y-1/2";
      tooltipArrowClasses =
        "-left-1 top-1/2 -translate-y-1/2 border-r-slate-900 border-b-0 border-t-0 border-l-0";
      break;
    case "top":
    default:
      tooltipPositionClasses = "bottom-full mb-2 left-1/2 -translate-x-1/2";
      tooltipArrowClasses =
        "-bottom-1 left-1/2 -translate-x-1/2 border-t-slate-900 border-b-0 border-l-0 border-r-0";
      break;
  }

  const ringClasses = isError ? "ring-rose-500/30" : "ring-emerald-500/30";

  const animationClasses = isError
    ? "motion-safe:animate-copy-shake"
    : "motion-safe:animate-copy-badge-pop";

  return (
    <div className="relative inline-flex items-center" data-copy-state={status}>
      <button
        type="button"
        onClick={handleCopy}
        data-state={status}
        aria-label={label ? `${label} to clipboard` : "Copy to clipboard"}
        title={!showTooltip ? activeLabel || "Copy" : undefined}
        className={`inline-flex min-h-[36px] items-center justify-center gap-1.5 rounded-lg px-3 py-1.5 text-xs font-medium transition-all duration-150 focus:outline-none focus:ring-2 focus:ring-cyan-500/50 disabled:cursor-not-allowed disabled:opacity-60 ${variantStyles} ${isCopied || isError ? activeStateStyles : ""} ${className}`}
      >
        {isCopied ? (
          <Check
            aria-hidden="true"
            className="shrink-0 text-emerald-400 motion-safe:animate-copy-check-pop"
            size={iconSize}
          />
        ) : isError ? (
          <AlertTriangle
            aria-hidden="true"
            className="shrink-0 text-rose-400"
            size={iconSize}
          />
        ) : (
          <Copy aria-hidden="true" className="shrink-0" size={iconSize} />
        )}
        {label && <span>{activeLabel}</span>}
      </button>

      {/* Floating Feedback Tooltip */}
      {showTooltip && (isCopied || isError) && (
        <div
          aria-hidden="true"
          className={`absolute z-[60] pointer-events-none whitespace-nowrap rounded-md bg-slate-900 px-2.5 py-1 text-xs font-semibold shadow-lg ring-1 ${ringClasses} transition-opacity animate-copy-fade-in ${tooltipPositionClasses}`}
        >
          <span
            className={`flex items-center gap-1.5 ${isError ? "text-rose-300" : "text-emerald-300"} ${animationClasses}`}
          >
            {isError ? (
              <AlertTriangle size={12} className="shrink-0" />
            ) : (
              <Check size={12} className="shrink-0" />
            )}
            {isError ? errorLabel : copiedLabel}
          </span>
          <div
            className={`absolute h-2 w-2 rotate-45 bg-slate-900 ${tooltipArrowClasses} ${ringClasses}`}
          />
        </div>
      )}

      {/* Screen reader announcements: success is polite, failure is assertive. */}
      <span className="sr-only" role="status" aria-live="polite">
        {isCopied ? copiedLabel : ""}
      </span>
      <span className="sr-only" role="alert" aria-live="assertive">
        {isError ? `${errorLabel}. ${errorMessage}` : ""}
      </span>
    </div>
  );
}
