export declare const RECONNECTABLE_CLOSE_CODES: readonly number[];
export declare const AUTH_FAILURE_CLOSE_CODE: 4003;
export function shouldReconnect(code: number): boolean;
export function isAuthFailure(code: number): boolean;
