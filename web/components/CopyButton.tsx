import React, { useState, useEffect, useRef } from "react";
import { Copy, Check } from "lucide-react";

export interface CopyButtonProps {
  text: string;
  label?: string;
  copiedLabel?: string;
  showTooltip?: boolean;
  tooltipPosition?: "top" | "bottom" | "left" | "right";
  timeout?: number;
  className?: string;
  iconSize?: number;
  variant?: "default" | "outline" | "ghost" | "icon";
  onCopy?: () => void;
}

export function CopyButton({
  text,
  label,
  copiedLabel = "Copied!",
  showTooltip = true,
  tooltipPosition = "top",
  timeout = 2000,
  className = "",
  iconSize = 16,
  variant = "ghost",
  onCopy,
}: CopyButtonProps) {
  const [copied, setCopied] = useState(false);
  const timeoutRef = useRef<NodeJS.Timeout | null>(null);

  useEffect(() => {
    return () => {
      if (timeoutRef.current) {
        clearTimeout(timeoutRef.current);
      }
    };
  }, []);

  const handleCopy = async (e: React.MouseEvent<HTMLButtonElement>) => {
    e.stopPropagation();
    try {
      if (navigator.clipboard && navigator.clipboard.writeText) {
        await navigator.clipboard.writeText(text);
      } else {
        // Fallback for non-HTTPS or unsupported environments
        const textArea = document.createElement("textarea");
        textArea.value = text;
        textArea.style.position = "fixed";
        textArea.style.opacity = "0";
        document.body.appendChild(textArea);
        textArea.select();
        document.execCommand("copy");
        document.body.removeChild(textArea);
      }

      setCopied(true);
      if (onCopy) onCopy();

      if (timeoutRef.current) {
        clearTimeout(timeoutRef.current);
      }

      timeoutRef.current = setTimeout(() => {
        setCopied(false);
      }, timeout);
    } catch (err) {
      console.error("Failed to copy text: ", err);
    }
  };

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

  // Tooltip position classes
  let tooltipPositionClasses = "";
  let tooltipArrowClasses = "";
  switch (tooltipPosition) {
    case "bottom":
      tooltipPositionClasses = "top-full mt-2 left-1/2 -translate-x-1/2";
      tooltipArrowClasses = "-top-1 left-1/2 -translate-x-1/2 border-t-0 border-b-slate-900";
      break;
    case "left":
      tooltipPositionClasses = "right-full mr-2 top-1/2 -translate-y-1/2";
      tooltipArrowClasses = "-right-1 top-1/2 -translate-y-1/2 border-r-0 border-l-slate-900";
      break;
    case "right":
      tooltipPositionClasses = "left-full ml-2 top-1/2 -translate-y-1/2";
      tooltipArrowClasses = "-left-1 top-1/2 -translate-y-1/2 border-l-0 border-r-slate-900";
      break;
    case "top":
    default:
      tooltipPositionClasses = "bottom-full mb-2 left-1/2 -translate-x-1/2";
      tooltipArrowClasses = "-bottom-1 left-1/2 -translate-x-1/2 border-b-0 border-t-slate-900";
      break;
  }

  return (
    <div className="relative inline-flex items-center">
      <button
        type="button"
        onClick={handleCopy}
        aria-label={copied ? copiedLabel : label || "Copy to clipboard"}
        title={!showTooltip ? (copied ? copiedLabel : label || "Copy") : undefined}
        className={`inline-flex min-h-[36px] items-center justify-center gap-1.5 rounded-lg px-3 py-1.5 text-xs font-medium transition-all duration-150 focus:outline-none focus:ring-2 focus:ring-cyan-500/50 ${variantStyles} ${className}`}
      >
        {copied ? (
          <Check className={`text-emerald-400 shrink-0`} size={iconSize} />
        ) : (
          <Copy className="shrink-0" size={iconSize} />
        )}
        {label && (
          <span>{copied ? copiedLabel : label}</span>
        )}
      </button>

      {/* Floating Feedback Tooltip */}
      {showTooltip && copied && (
        <div
          role="status"
          aria-live="polite"
          className={`absolute z-50 pointer-events-none flex items-center gap-1.5 whitespace-nowrap rounded-md bg-slate-900 px-2.5 py-1 text-xs font-semibold text-emerald-400 shadow-lg ring-1 ring-emerald-500/30 transition-opacity animate-in fade-in duration-150 ${tooltipPositionClasses}`}
        >
          <Check size={12} className="text-emerald-400" />
          <span>{copiedLabel}</span>
          <div
            className={`absolute h-2 w-2 rotate-45 bg-slate-900 border-emerald-500/30 ${tooltipArrowClasses}`}
          />
        </div>
      )}
    </div>
  );
}
