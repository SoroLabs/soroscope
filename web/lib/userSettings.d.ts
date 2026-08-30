export declare const SETTINGS_STORAGE_KEY: string;
export declare const MIN_TIMEOUT_MS: number;
export declare const MAX_TIMEOUT_MS: number;

export interface UserSettings {
  /** Custom Soroban RPC endpoint; empty string means "use the network default". */
  rpcUrl: string;
  /** Custom indexer/analyzer backend base URL; empty string means "use NEXT_PUBLIC_API_URL". */
  indexerUrl: string;
  requestTimeoutMs: number;
}

export declare const DEFAULT_SETTINGS: UserSettings;

export interface EndpointValidation {
  valid: boolean;
  normalized: string;
  error: string | null;
}

export declare function validateEndpointUrl(
  value: string,
  options?: { allowEmpty?: boolean },
): EndpointValidation;

export declare function validateSettings(raw: Partial<UserSettings>): {
  valid: boolean;
  errors: Partial<Record<keyof UserSettings, string>>;
};

export declare function normalizeSettings(raw: unknown): UserSettings;

/** A minimal Storage-compatible interface so tests can inject a fake. */
export interface SettingsStorage {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
  removeItem(key: string): void;
}

export declare function loadSettings(storage?: SettingsStorage): UserSettings;
export declare function saveSettings(
  settings: Partial<UserSettings>,
  storage?: SettingsStorage,
): UserSettings;
export declare function clearSettings(storage?: SettingsStorage): UserSettings;
export declare function resolveEndpoint(customUrl: string, fallbackUrl: string): string;
