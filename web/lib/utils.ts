import { type ClassValue, clsx } from "clsx";
import { twMerge } from "tailwind-merge";

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

const BASE64_ALPHABET =
  "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

function encodeBase64Chunk(bytes: Uint8Array): string {
  let result = "";
  let i = 0;

  for (; i + 2 < bytes.length; i += 3) {
    const chunk = (bytes[i] << 16) | (bytes[i + 1] << 8) | bytes[i + 2];
    result +=
      BASE64_ALPHABET[(chunk >> 18) & 0x3f] +
      BASE64_ALPHABET[(chunk >> 12) & 0x3f] +
      BASE64_ALPHABET[(chunk >> 6) & 0x3f] +
      BASE64_ALPHABET[chunk & 0x3f];
  }

  const remaining = bytes.length - i;
  if (remaining === 1) {
    const chunk = bytes[i] << 16;
    result +=
      BASE64_ALPHABET[(chunk >> 18) & 0x3f] +
      BASE64_ALPHABET[(chunk >> 12) & 0x3f] +
      "==";
  } else if (remaining === 2) {
    const chunk = (bytes[i] << 16) | (bytes[i + 1] << 8);
    result +=
      BASE64_ALPHABET[(chunk >> 18) & 0x3f] +
      BASE64_ALPHABET[(chunk >> 12) & 0x3f] +
      BASE64_ALPHABET[(chunk >> 6) & 0x3f] +
      "=";
  }

  return result;
}

/**
 * Converts an ArrayBuffer to Base64 in chunks to avoid large argument limits.
 */
export function arrayBufferToBase64(
  buffer: ArrayBuffer,
  chunkSizeBytes = 32 * 1024
): string {
  const bytes = new Uint8Array(buffer);
  if (bytes.length === 0) return "";

  // Keep chunk size aligned to 3-byte groups for exact base64 boundaries.
  const normalizedChunkSize = Math.max(
    3,
    chunkSizeBytes - (chunkSizeBytes % 3)
  );

  const encodedChunks: string[] = [];
  for (let offset = 0; offset < bytes.length; offset += normalizedChunkSize) {
    const end = Math.min(offset + normalizedChunkSize, bytes.length);
    encodedChunks.push(encodeBase64Chunk(bytes.subarray(offset, end)));
  }

  return encodedChunks.join("");
}
