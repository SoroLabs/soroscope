export declare const DEFAULT_MAX_LENGTH: number;
export declare const MAX_MERMAID_LENGTH: number;

/** Escape a value for safe interpolation into an HTML context. */
export declare function escapeHtml(value: unknown): string;

/** Remove control characters from a string. */
export declare function stripControlChars(value: unknown): string;

/** Sanitize a free-text field for display/transport with a length cap. */
export declare function sanitizePlainText(value: unknown, maxLength?: number): string;

/** Sanitize a mermaid definition before rendering it into the DOM. */
export declare function sanitizeMermaidDefinition(value: unknown): string;

/** Sanitize a user-typed Soroban contract ID (returns '' when invalid). */
export declare function sanitizeContractId(value: unknown): string;

/** True when the value is a 56-char Stellar/Soroban strkey (G.../C.../A...). */
export declare function isValidContractId(value: unknown): boolean;