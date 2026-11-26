'use client';

import React, { useCallback, useEffect, useState } from 'react';
import { Keyboard, X } from 'lucide-react';

interface ShortcutRow {
  keys: string[];
  description: string;
}

const SHORTCUTS: ShortcutRow[] = [
  { keys: ['Cmd', 'K'], description: 'Open global search' },
  { keys: ['Cmd', 'Enter'], description: 'Run simulation' },
  { keys: ['Esc'], description: 'Close modal / sidebar' },
  { keys: ['?'], description: 'Open this keyboard shortcuts cheat sheet' },
];

/**
 * App-wide keyboard shortcuts cheat sheet modal.
 *
 * Mounted once in `_app` alongside `GlobalSearchModal`.
 * Pressing `?` anywhere on the page (outside an input / textarea) opens the
 * overlay.  Pressing `Esc` or clicking the backdrop closes it.
 */
export function KeyboardShortcutsModal() {
  const [open, setOpen] = useState(false);

  const close = useCallback(() => setOpen(false), []);

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      // Never steal `?` from text inputs or textareas.
      const tag = (event.target as HTMLElement)?.tagName?.toLowerCase();
      const isEditable =
        tag === 'input' || tag === 'textarea' || (event.target as HTMLElement)?.isContentEditable;

      if (event.key === '?' && !isEditable && !event.metaKey && !event.ctrlKey && !event.altKey) {
        event.preventDefault();
        setOpen((prev) => !prev);
        return;
      }

      if (event.key === 'Escape' && open) {
        event.preventDefault();
        close();
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [open, close]);

  // Lock background scroll while open.
  useEffect(() => {
    if (!open) {
      document.body.style.overflow = '';
      return;
    }
    document.body.style.overflow = 'hidden';
    return () => {
      document.body.style.overflow = '';
    };
  }, [open]);

  if (!open) return null;

  return (
    <div
      className="fixed inset-0 z-[100] flex items-center justify-center px-4"
      role="dialog"
      aria-modal="true"
      aria-label="Keyboard shortcuts"
    >
      {/* Backdrop */}
      <div
        className="fixed inset-0 bg-slate-950/80 backdrop-blur-sm"
        onClick={close}
        aria-hidden="true"
      />

      {/* Panel */}
      <div className="relative z-10 w-full max-w-md overflow-hidden rounded-2xl border border-slate-700 bg-slate-900 shadow-2xl">
        {/* Header */}
        <div className="flex items-center justify-between border-b border-slate-800 px-5 py-4">
          <div className="flex items-center gap-2.5">
            <Keyboard className="h-4 w-4 text-cyan-400" aria-hidden="true" />
            <h2 className="text-sm font-semibold text-slate-100">Keyboard Shortcuts</h2>
          </div>
          <button
            type="button"
            onClick={close}
            aria-label="Close keyboard shortcuts"
            className="rounded p-1 text-slate-500 transition-colors hover:text-slate-200"
          >
            <X className="h-4 w-4" />
          </button>
        </div>

        {/* Shortcut list */}
        <ul className="divide-y divide-slate-800/60 px-5 py-2">
          {SHORTCUTS.map(({ keys, description }) => (
            <li key={description} className="flex items-center justify-between gap-4 py-3">
              <span className="text-sm text-slate-300">{description}</span>
              <span className="flex shrink-0 items-center gap-1">
                {keys.map((key, i) => (
                  <React.Fragment key={key}>
                    {i > 0 && <span className="text-xs text-slate-600">+</span>}
                    <kbd className="inline-flex items-center rounded border border-slate-600 bg-slate-800 px-1.5 py-0.5 font-mono text-[11px] font-medium text-slate-300 shadow-sm">
                      {key}
                    </kbd>
                  </React.Fragment>
                ))}
              </span>
            </li>
          ))}
        </ul>

        {/* Footer hint */}
        <div className="border-t border-slate-800 px-5 py-3 text-[11px] text-slate-500">
          Press <kbd className="rounded border border-slate-700 bg-slate-800 px-1 py-0.5 font-mono text-[10px]">?</kbd> or{' '}
          <kbd className="rounded border border-slate-700 bg-slate-800 px-1 py-0.5 font-mono text-[10px]">Esc</kbd> to close
        </div>
      </div>
    </div>
  );
}

export default KeyboardShortcutsModal;
