import type { ContractFunction } from './sorobantypes';

export interface SearchCommand {
  id: string;
  title: string;
  subtitle?: string;
  group: string;
  /** Route to navigate to when selected. */
  href?: string;
  /** Non-navigation action identifier handled by the overlay. */
  action?: string;
  payload?: Record<string, unknown>;
  keywords?: string[];
}

export declare const BASE_COMMANDS: SearchCommand[];

/** True when the event is Cmd+K / Ctrl+K. */
export declare function isSearchShortcut(event: KeyboardEvent | null | undefined): boolean;

/** True when the event is Escape. */
export declare function isDismissShortcut(event: KeyboardEvent | null | undefined): boolean;

export declare function scoreCommand(command: SearchCommand, query: string): number;

export declare function filterCommands(
  commands: SearchCommand[],
  query: string,
  options?: { limit?: number },
): SearchCommand[];

export declare function buildCommandRegistry(context?: {
  functions?: ContractFunction[];
  extra?: SearchCommand[];
}): SearchCommand[];

export declare function moveHighlight(current: number, delta: number, length: number): number;
