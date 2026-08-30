# Local storage encryption

Sensitive client-session values must be accessed through
`lib/encryptedStorage`, not through `localStorage` directly.

The helper encrypts each value with AES-256-GCM and a fresh 96-bit IV. Its
non-exportable key is generated with the Web Crypto API and retained only in
memory. Consequently, encrypted values can be read only for the lifetime of
the current page session. Stale, malformed, or unauthenticated entries are
removed when read.

This reduces accidental plaintext exposure at rest. It does not protect data
from a malicious script executing in the same page context, because such a
script can access application memory and invoke browser APIs.
