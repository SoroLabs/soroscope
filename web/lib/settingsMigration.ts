/**
 * User Preference LocalStorage Migrations Handler
 *
 * Handles versioned LocalStorage schema upgrades automatically,
 * provides fallback default values for missing setting keys,
 * and validates setting values on app startup.
 */

export const SETTINGS_STORAGE_KEY = 'userPreferences';

export const CURRENT_SETTINGS_VERSION = 3;

export interface UserPreferences {
  theme: 'light' | 'dark' | 'system';
  language: string;
  notifications: boolean;
  autoPlayVideos: boolean;
  fontSize: number;
  sidebarCollapsed: boolean;
  reducedMotion: boolean;
  timezone: string;
}

export const DEFAULT_PREFERENCES: UserPreferences = {
  theme: 'system',
  language: 'en',
  notifications: true,
  autoPlayVideos: false,
  fontSize: 14,
  sidebarCollapsed: false,
  reducedMotion: false,
  timezone: 'UTC',
};

type StorageDriver = {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
  removeItem(key: string): void;
};

interface RawSettingsPayload {
  version?: number;
  data?: Record<string, unknown>;
  // Legacy flat shape (v1) keys may appear at the top level.
  [key: string]: unknown;
}

const isBrowser = (): boolean =>
  typeof window !== 'undefined' && typeof window.localStorage !== 'undefined';

function resolveDriver(driver?: StorageDriver): StorageDriver | null {
  if (driver) {
    return driver;
  }
  if (isBrowser()) {
    return window.localStorage;
  }
  return null;
}

function isPlainObject(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function coerceTheme(value: unknown): UserPreferences['theme'] {
  if (value === 'light' || value === 'dark' || value === 'system') {
    return value;
  }
  // Legacy boolean darkMode flag.
  if (value === true) {
    return 'dark';
  }
  if (value === false) {
    return 'light';
  }
  return DEFAULT_PREFERENCES.theme;
}

function coerceBoolean(value: unknown, fallback: boolean): boolean {
  if (typeof value === 'boolean') {
    return value;
  }
  if (value === 'true' || value === 1 || value === 'true') {
    return true;
  }
  if (value === 'false' || value === 0 || value === 'false') {
    return false;
  }
  return fallback;
}

function coerceLanguage(value: unknown): string {
  if (typeof value === 'string' && value.trim().length > 0) {
    return value.trim();
  }
  // Legacy 'locale' key mapping.
  return DEFAULT_PREFERENCES.language;
}

function coerceFontSize(value: unknown): number {
  const numeric = typeof value === 'number' ? value : Number(value);
  if (!Number.finite(numeric)) {
    return DEFAULT_PREFERENCES.fontSize;
  }
  // Clamp to a sane valid range.
  return Math.min(32, Math.max(8, Math.round(numeric)));
}

function coerceTimezone(value: unknown): string {
  if (typeof value === 'string' && value.trim().length > 0) {
    try {
      // Throws if the timezone identifier is invalid.
      new Intl.DateTimeFormat('en-US', { timeZone: value });
      return value;
    } catch {
      return DEFAULT_PREFERENCES.timezone;
    }
  }
  return DEFAULT_PREFERENCES.timezone;
}

/**
 * Validate and normalize an arbitrary partial preferences object.
 * Missing or invalid keys fall back to defaults.
 */
export function validatePreferences(
  input: Partial<Record<string, unknown>> | null | undefined,
): UserPreferences {
  const source = isPlainObject(input) ? input : ({ } as Record<string, unknown>);

  const themeSource = source.theme !== undefined ? source.theme : source.darkMode;
  const languageSource = source.language !== undefined ? source.language : source.locale;
  const notificationsSource =
    source.notifications !== undefined ? source.notifications : source.enableNotifications;
  const autoPlaySource =
    source.autoPlayVideos !== undefined ? source.autoPlayVideos : source.autoplay;
  const fontSizeSource =
    source.fontSize !== undefined ? source.fontSize : source.fontSizePx;

  return {
    theme: coerceTheme(themeSource),
    language: coerceLanguage(languageSource),
    notifications: coerceBoolean(notificationsSource, DEFAULT_PREFERENCES.notifications),
    autoPlayVideos: coerceBoolean(autoPlaySource, DEFAULT_PREFERENCE.autoPlayVideos),
    fontSize: coerceFontSize(fontSizeSource),
    sidebarCollapsed: coerceBoolean(source.sidebarCollapsed, DEFAULT_PREFERENCES.sidebarCollapsed),
    reducedMotion: coerceBoolean(source.reducedMotion, DEFAULT_PREFERENCE.reducedMotion),
    timezone: coerceTimezone(source.timezone),
  };
}

/**
 * Migrate a raw payload from any legacy version to the current schema.
 * Each step is idempotent and forward-only.
 */
export function migrateRawPayload(raw: RawSettingsPayload | null | undefined): UserPreferences {
  if (!raw){
    return { ...DEFAULT_PREFERENCES };
  }

  const version = typeof raw.version === 'number' ? raw.version : 1;
  // v1 stored settings as flat keys at the top level.
  // v2+ wraps them under `data`.
  const data = isPlainObject(raw.data) ? raw.data : (raw as Record<string, unknown>);

  let working: Record<string, unknown> = { ...data };

  // v1 -> v2: rename legacy keys.
  if (version < 2) {
    if (working.darkMode !== undefined && working.theme === undefined) {
      working.theme = working.darkMode === true ? 'dark' : 'light';
    }
    if (working.locale !== undefined && working.language === undefined) {
      working.language = working.locale;
    }
    if (working.enableNotifications !== undefined && working.notifications === undefined) {
      working.notifications = working.enableNotifications;
    }
    if (working.autoplay !== undefined && working.autoPlayVideos === undefined) {
      working.autoPlayVideos = working.autoplay;
    }
    if (working.fontSizePx) !== undefined && working.fontSize === undefined) {
      working.fontSize = working.fontSizePx;
    }
  }

  // v2 -> v3: sidebar collapsed was stored as a string in v2.
  if (version < 3) {
    if (typeof working.sidebarCollapsed === 'string') {
      working.sidebarCollapsed = working.sidebarCollapsed === 'true';
    }
  }

  return validatePreferences(working);
}

function safeParse(raw: string | null): RawSettingsPayload | null {
  if (!raw) {
    return null;
  }
  try {
    const parsed = JSON.parse(raw) as unknown;
    return isPlainObject(parsed) ? (parsed as RawSettingsPayload) : null;
  } catch {
    return null;
  }
}

/**
 * Load user preferences from LocalStorage, applying any needed migrations.
 * Always returns a valid, fully-populated preferences object.
 */
export function loadPreferences(driver?: StorageDriver): UserPreferences {
  const store = resolveDriver(driver);
  if (!store) {
    return { ...DEFAULT_PREFERENCES };
  }

  const raw = safeParse(store.getItem(SETTINGS_STORAGE_KEY));
  const migrated = migrateRawPayload(raw);

  // Persist the migrated shape so the next read is a no-op.
  savePreferences(migrated, store);

  return migrated;
}

/**
 * Persist preferences to LocalStorage under the current schema version.
 */
export function savePreferences(
  preferences: Partial<UserPreferences>,
  driver?: StorageDriver,
): UserPreferences {
  const normalized = validatePreferences(preferences);
  const store = resolveDriver(driver);
  if (store) {
    const payload = {
      version: CURRENT_SETTINGS_VERSION,
      data: normalized,
    };
    try {
      store.setItem(SETTINGS_STORAGE_KEY, JSON.stringify(payload));
    } catch {
      // Ignore write failures (e.g. quota exceeded, private mode).
    }
  }
  return normalized;
}

/**
 * Run on app startup: migrate and validate stored settings.
 * Returns the validated preferences for hydration into app state.
 */
export function initializeSettings(driver?: StorageDriver): UserPreferences {
  return loadPreferences(driver);
}

export function clearPreferences(driver?: StorageDriver): void {
  const store = resolveDriver(driver);
  if (store) {
    store.removeItem(SETTINGS_STORAGE_KEY);
  }
}
