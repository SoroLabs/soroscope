# WASM Error Handling - Test Guide

## Test Scenarios

This guide outlines how to test the new WASM error handling system with descriptive backend error messages.

### Prerequisites

1. **Backend Running**:
   ```bash
   cd soroscope/core
   RUST_LOG=info cargo run -p soroscope-core
   ```
   Listens on `http://localhost:8080`

2. **Frontend Running**:
   ```bash
   cd soroscope/web
   npm run dev
   ```
   Accessible at `http://localhost:3000`

### Test Case 1: Valid WASM Upload

**Objective**: Verify successful upload and validation

**Steps**:
1. Navigate to `http://localhost:3000`
2. Drag and drop a valid compiled Soroban contract (.wasm file)
3. Observe upload progression:
   - "Scanning contract..." (client-side validation)
   - "Validating with server..." (backend validation)
   - ✓ "Contract uploaded successfully" (green checkmark)

**Expected Result**: 
- File accepted with green success state
- File info displayed (name, size)
- "Upload a different file" option appears

---

### Test Case 2: Invalid File Type

**Objective**: Verify rejection of non-WASM files

**Steps**:
1. Create a dummy file: `echo "test" > test.txt`
2. Drag and drop `test.txt` into upload zone
3. Observe error message

**Expected Error**:
```
❌ Invalid File Type
"test.txt" was rejected — only .wasm files are accepted (got .txt)
💡 Please upload a compiled .wasm file.
```

**Expected Result**: Red error state with Try again button

---

### Test Case 3: Invalid Base64 Encoding

**Objective**: Test backend error for corrupted base64

**Steps**:
1. Modify the upload component temporarily to send invalid base64
2. Upload a file
3. Observe backend validation error

**Backend Command** (for direct testing):
```bash
curl -X POST http://localhost:8080/analyze/wasm \
  -H "Content-Type: application/json" \
  -d '{
    "wasm_bytes": "invalid!!!base64",
    "function_name": "test",
    "args": []
  }'
```

**Expected Error Response**:
```
❌ Invalid WASM Encoding
The file appears to be corrupted or improperly encoded. 
Ensure you're uploading a valid compiled Soroban contract.
💡 Please check your contract and try uploading again.
```

**Expected Result**: Red error state with technical details (if enabled)

---

### Test Case 4: Malformed WASM Binary

**Objective**: Test backend error for invalid WASM structure

**Steps**:
1. Create a minimal WASM-like file with invalid structure:
   ```bash
   # Create file with correct magic number but invalid content
   printf '\x00\x61\x73\x6d\xff\xff\xff\xff' > bad.wasm
   ```
2. Upload the file
3. Observe backend validation error

**Expected Error**:
```
❌ Invalid WASM Format
This doesn't appear to be a valid WebAssembly module. 
Make sure you're uploading a compiled .wasm file from Soroban.
💡 Please ensure you're uploading a valid compiled Soroban contract.
```

**Expected Result**: Red error state with Try again button

---

### Test Case 5: WASM Version Mismatch

**Objective**: Test error when WASM version doesn't match

**Steps**:
1. Create a WASM file with version mismatch:
   ```bash
   # Magic number (0x0061736d) + version 2 instead of 1
   printf '\x00\x61\x73\x6d\x02\x00\x00\x00' > wrong_version.wasm
   ```
2. Upload the file
3. Observe client-side validation error

**Expected Client-side Error**:
```
❌ Invalid WASM File
Unsupported WASM version: 2. Expected version 1
💡 Please ensure you're uploading a valid compiled Soroban contract.
```

**Expected Result**: Red error state during scanning phase

---

### Test Case 6: Oversized WASM File

**Objective**: Test 413 error for file size limits

**Steps**:
1. Create a large WASM file beyond backend limits (if configured)
2. Upload the file
3. Observe backend error

**Backend Command** (simulated):
```bash
# Server returns 413 if file/WASM is too large
```

**Expected Error**:
```
❌ File Too Large
The WASM file is too large for analysis.
💡 Optimize your contract to reduce its size.
```

---

### Test Case 7: Server Unavailable (503)

**Objective**: Test error handling when backend is down

**Steps**:
1. Stop the backend server
2. Try uploading a valid WASM file
3. Observe error message

**Expected Error**:
```
❌ Service Unavailable
The analysis service is temporarily unavailable.
💡 Please try again in a few moments.
```

**Expected Result**: User is informed to retry later

---

### Test Case 8: Internal Server Error (500)

**Objective**: Test generic server error handling

**Steps**:
1. Backend should return 500 for unrecoverable errors
2. Upload a file that triggers backend failure
3. Observe error message

**Expected Error**:
```
❌ Server Error
The backend encountered an error while analyzing your contract.
💡 Please try again later.
```

---

### Test Case 9: Unauthorized (401)

**Objective**: Test authentication error (if auth is implemented)

**Steps**:
1. Remove or invalidate any authentication token
2. Upload a file
3. Observe 401 error response

**Expected Error**:
```
❌ Unauthorized
You don't have permission to analyze contracts.
💡 Please connect your wallet and try again.
```

---

### Test Case 10: Network Timeout

**Objective**: Test handling of network failures

**Steps**:
1. Use browser dev tools to throttle network to offline
2. Try uploading a file
3. Observe error handling

**Expected Error**:
```
❌ Validation Error
Failed to validate with backend
💡 Please try uploading again.
```

---

## UI Validation Checklist

### Upload Zone States
- [ ] **Idle**: Gray border, drag & drop prompt visible
- [ ] **Hover**: Blue highlight, "Release to upload" text
- [ ] **Scanning**: Violet animation, file info displayed
- [ ] **Submitting**: Violet animation, "Validating with server..." text
- [ ] **Success**: Green checkmark, file info card, "Upload a different file" link
- [ ] **Error**: Red border, error title, error message, suggestion, "Try again" button

### Error Display Elements
- [ ] Error title/icon displayed prominently
- [ ] Error message is user-friendly (not raw JSON)
- [ ] Technical details section exists (hidden by default)
- [ ] Suggested action displayed when available
- [ ] Try again button functional
- [ ] Reset clears all error state

### File Input Elements
- [ ] Click to browse works
- [ ] Drag and drop works
- [ ] Only .wasm files accepted in file picker
- [ ] File size validation works

---

## Performance Testing

### Upload Speed
1. Test with files of various sizes:
   - Small: 50KB
   - Medium: 500KB
   - Large: 2MB

2. Measure time in each state:
   - Scanning: ~1 second
   - Submitting: varies with backend

### Concurrent Uploads
1. Open multiple browser tabs
2. Attempt simultaneous uploads
3. Verify each upload processes independently

---

## Error Pattern Testing

### Custom Error Messages
Test that backend custom errors are properly mapped:

```bash
# Test case: backend returns custom error
curl -X POST http://localhost:8080/analyze/wasm \
  -H "Content-Type: application/json" \
  -d '{
    "wasm_bytes": "<valid base64>",
    "function_name": "export_that_doesnt_exist",
    "args": []
  }' | jq
```

Expected: Error pattern for "function not found" is triggered

---

## Accessibility Testing

- [ ] Keyboard navigation works (Tab through elements)
- [ ] Error messages readable by screen readers
- [ ] Color not sole indicator (icons used)
- [ ] Error messages have sufficient contrast

---

## Browser Compatibility

Test in:
- [ ] Chrome 90+
- [ ] Firefox 88+
- [ ] Safari 14+
- [ ] Edge 90+

---

## Regression Testing

After any backend changes:
1. Re-run all test cases
2. Check error message patterns still match
3. Verify backend logs show expected error types
4. Confirm UI reflects backend changes

---

## Debugging Tips

### Enable Debug Logging
```tsx
// Add to upload-zone.tsx
if (process.env.DEBUG_WASM_UPLOAD === 'true') {
  console.log('[DEBUG] Upload state:', uploadState);
  console.log('[DEBUG] Error details:', errorDetails);
}
```

### Check Backend Logs
```bash
# In backend terminal with RUST_LOG set
# Look for error messages like:
# thread 'tokio-runtime-worker' error: Invalid base64 WASM data
```

### Network Inspector
Use Browser DevTools Network tab to:
1. Inspect request body (base64 truncated)
2. Check response headers
3. Review response body for error details
4. Monitor timing of submitting state

### React DevTools
Inspect component props:
1. Check `uploadState` value
2. Verify `errorDetails` object structure
3. Confirm callbacks are fired

