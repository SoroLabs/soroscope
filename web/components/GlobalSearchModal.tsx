'use client';

import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useRouter } from 'next/router';
import { CornerDownLeft, Search, X } from 'lucide-react';

import {
  buildCommandRegistry,
  filterCommands,
  isDismissShortcut,
  isSearchShortcut,
  moveHighlight,
} from '../lib/searchCommands';
import type { SearchCommand } from '../lib/searchCommands';
import { MOCK_CONTRACT_FUNCTIONS } from '../lib/sorobantypes';

/**
 * Custom event fired when a non-navigation command is picked, so pages can
 * react without the modal needing to know about their internal state.
 */
export const SEARCH_COMMAND_EVENT = 'soroscope:search-command';

/**
 * App-wide quick search overlay.
 *
 * Mounted once in `_app`, so Cmd+K / Ctrl+K works on every page without moving
 * the mouse to the header.
 */
export function GlobalSearchModal() {
  const router = useRouter();
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState('');
  const [highlight, setHighlight] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);

  const commands = useMemo(
    () => buildCommandRegistry({ functions: MOCK_CONTRACT_FUNCTIONS }),
    [],
  );
  const results = useMemo(() => filterCommands(commands, query), [commands, query]);

  const close = useCallback(() => {
    setOpen(false);
    setQuery('');
    setHighlight(0);
  }, []);

  const runCommand = useCallback(
    (command: SearchCommand | undefined) => {
      if (!command) return;
      close();

      if (command.href) {
        void router.push(command.href);
        return;
      }

      if (command.action && typeof window !== 'undefined') {
        window.dispatchEvent(
          new CustomEvent(SEARCH_COMMAND_EVENT, {
            detail: { action: command.action, payload: command.payload },
          }),
        );
      }
    },
    [close, router],
  );

  // Global shortcut listener — intentionally on window so it fires regardless
  // of which element currently has focus.
  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (isSearchShortcut(event)) {
        event.preventDefault();
        setOpen((prev) => !prev);
        return;
      }
      if (isDismissShortcut(event)) {
        setOpen((prev) => (prev ? false : prev));
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, []);

  // Focus the input and lock background scroll while the overlay is up.
  useEffect(() => {
    if (!open) {
      document.body.style.overflow = '';
      return;
    }

    document.body.style.overflow = 'hidden';
    const timer = window.setTimeout(() => inputRef.current?.focus(), 0);
    return () => {
      window.clearTimeout(timer);
      document.body.style.overflow = '';
    };
  }, [open]);

  useEffect(() => {
    setHighlight(0);
  }, [query]);

  if (!open) return null;

  const handleInputKeyDown = (event: React.KeyboardEvent<HTMLInputElement>) => {
    if (event.key === 'ArrowDown') {
      event.preventDefault();
      setHighlight((current) => moveHighlight(current, 1, results.length));
    } else if (event.key === 'ArrowUp') {
      event.preventDefault();
      setHighlight((current) => moveHighlight(current, -1, results.length));
    } else if (event.key === 'Enter') {
      event.preventDefault();
      runCommand(results[highlight]);
    }
  };

  return (
    <div
      className="fixed inset-0 z-[100] flex items-start justify-center px-4 pt-[12vh]"
      role="dialog"
      aria-modal="true"
      aria-label="Global search"
    >
      <div
        className="fixed inset-0 bg-slate-950/80 backdrop-blur-sm"
        onClick={close}
        aria-hidden="true"
      />

      <div className="relative z-10 w-full max-w-xl overflow-hidden rounded-2xl border border-slate-700 bg-slate-900 shadow-2xl">
        <div className="flex items-center gap-3 border-b border-slate-800 px-4 py-3">
          <Search className="h-4 w-4 shrink-0 text-slate-500" aria-hidden="true" />
          <input
            ref={inputRef}
            type="text"
            role="combobox"
            aria-expanded="true"
            aria-controls="global-search-results"
            aria-autocomplete="list"
            placeholder="Search pages, functions and settings..."
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={handleInputKeyDown}
            className="w-full bg-transparent text-sm text-slate-100 placeholder:text-slate-500 focus:outline-none"
          />
          <button
            type="button"
            onClick={close}
            aria-label="Close search"
            className="shrink-0 rounded p-1 text-slate-500 transition-colors hover:text-slate-200"
          >
            <X className="h-4 w-4" />
          </button>
        </div>

        <ul id="global-search-results" role="listbox" className="max-h-80 overflow-y-auto py-2">
          {results.length === 0 && (
            <li className="px-4 py-6 text-center text-sm text-slate-500">
              No matches for &ldquo;{query}&rdquo;
            </li>
          )}

          {results.map((command, index) => (
            <li key={command.id} role="option" aria-selected={index === highlight}>
              <button
                type="button"
                onMouseEnter={() => setHighlight(index)}
                onClick={() => runCommand(command)}
                className={`flex w-full items-center justify-between gap-3 px-4 py-2.5 text-left transition-colors ${
                  index === highlight ? 'bg-cyan-500/10' : 'hover:bg-slate-800/60'
                }`}
              >
                <span className="min-w-0">
                  <span
                    className={`block truncate text-sm font-medium ${
                      index === highlight ? 'text-cyan-300' : 'text-slate-200'
                    }`}
                  >
                    {command.title}
                  </span>
                  {command.subtitle && (
                    <span className="block truncate text-xs text-slate-500">
                      {command.subtitle}
                    </span>
                  )}
                </span>
                <span className="shrink-0 text-[10px] uppercase tracking-wide text-slate-500">
                  {command.group}
                </span>
              </button>
            </li>
          ))}
        </ul>

        <div className="flex items-center justify-between border-t border-slate-800 px-4 py-2 text-[11px] text-slate-500">
          <span className="flex items-center gap-1.5">
            <CornerDownLeft className="h-3 w-3" /> to select &bull; ↑↓ to navigate
          </span>
          <span>esc to close</span>
        </div>
      </div>
    </div>
  );
}

export default GlobalSearchModal;
