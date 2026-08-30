# Implementation Plan ✅

## Step 1: Create OfflineBanner.tsx ✅
- [x] Component monitoring navigator.onLine and online/offline events
- [x] Shows offline warning when disconnected
- [x] Shows "Back online!" success state with auto-dismiss (3s)
- [x] Dismissible via close button
- [x] Framer-motion animations
- [x] Dark theme styling

## Step 2: Create SyntaxHighlighter.tsx ✅
- [x] Regex-based tokenizer (sticky regex) for Rust/Soroban contract source code
- [x] Regex-based tokenizer (sticky regex) for XDR format (hex, base64, field names, punctuation)
- [x] Colored spans matching dark theme (cyan keywords, yellow types, green strings, orange numbers)
- [x] Optional line numbers with row hover highlighting
- [x] Copy button integration (reusing existing CopyButton)
- [x] Header bar with language badge, line count
- [x] Dark theme optimized

## Step 3: Edit _app.tsx ✅
- [x] Import and render OfflineBanner inside WalletProvider

## Step 4: Edit index.tsx ✅
- [x] Import SyntaxHighlighter
- [x] Add collapsible Contract Source & XDR section with toggle between modes
- [x] Sample Rust/Soroban contract source code for demo
- [x] Sample base64 XDR data for demo

