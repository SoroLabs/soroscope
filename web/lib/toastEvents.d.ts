import type { ToastItem, ToastType } from '../components/Toast';

export declare const TOAST_EVENT: 'soroscope:toast';

export interface ToastRequest {
  message: string;
  type?: ToastType;
}

export function createToast(toast: ToastRequest): ToastItem;

export function emitToast(toast: ToastRequest, target?: EventTarget): boolean;
