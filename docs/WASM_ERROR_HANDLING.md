# WASM Error Handling & Validation Guide

## Overview

SoroScope now provides comprehensive error handling for WASM file uploads and backend validation. When the backend rejects a WASM file, users see detailed, actionable error messages describing what went wrong and how to fix it.

## Architecture

### Frontend Error Handling Flow

1. **Client-side Validation** (upload-zone.tsx)
   - File type validation (.wasm only)
   - Magic number check (0x0061736d)
   - WASM version validation (v1)
   - User-friendly rejection messages

2. **Backend Validation** (optional, enabled by default)
   - WASM bytes sent as base64-encoded JSON
   - Backend validates structural integrity
   - Compatibility checks performed

3. **Error Mapping** (errorHandling.ts)
   - Backend errors categorized by pattern
   - HTTP status codes mapped to user-friendly messages
   - Suggested actions provided

### Error Categories

The system recognizes these WASM-specific errors:

| Error Type | Trigger Pattern | User Message | Action |
|------------|-----------------|--------------|--------|
| Invalid Encoding | `base64`, `decode`, `corrupted` | WASM appears corrupted | Verify compilation process |
| Invalid Format | `malformed`, `not valid` | Not a valid WASM module | Check Soroban compilation |
| Unsupported Version | `version`, `unsupported` | WASM version incompatible | Recompile with compatible version |
| Resource Exceeded | `memory`, `limit`, `overflow` | Contract too complex | Simplify or split modules |
| Timeout | `timeout`, `took too long` | Analysis timed out | Retry or simplify contract |
| Function Not Found | `function`, `export`, `not found` | Function not exported | Check contract exports |

## Usage

### Basic Upload with Backend Validation

```tsx
import { UploadZone } from '@/components/upload-zone';

export default function MyPage() {
  return (
    <UploadZone
      onFileReady={(file) => {
        console.log('WASM file ready:', file.name);
        // Send to analysis or process further
      }}
      enableBackendValidation={true}
      backendUrl="http://localhost:8080/analyze/wasm"
    />
  );
}
```

### Disable Backend Validation (Client-side Only)

```tsx
<UploadZone
  enableBackendValidation={false}
  onFileReady={handleFileReady}
/>
```

### Custom Backend Endpoint

```tsx
<UploadZone
  backendUrl="https://api.example.com/validate-wasm"
  onFileReady={handleFileReady}
/>
```

## Error Display

### Error Details in UI

When a WASM file is rejected, the UI displays:

1. **Error Title** - Brief categorization (e.g., "Invalid WASM Format")
2. **Error Message** - User-friendly explanation
3. **Technical Details** (collapsible) - Raw error context
4. **Suggested Action** - How to resolve the issue

Example:
```
❌ Invalid WASM Encoding

The file appears to be corrupted or improperly encoded. 
Ensure you're uploading a valid compiled Soroban contract.

💡 Try uploading a different contract or check the build logs.
```

## Backend Integration

### Request Format

```json
{
  "wasm_bytes": "<base64-encoded WASM binary>",
  "function_name": "main",
  "args": []
}
```

### Error Response Format

The backend should return errors in this format for best UI integration:

```json
{
  "error": "BAD_REQUEST",
  "message": "Invalid base64 WASM data: unexpected character at line 1"
}
```

or with a more specific message:

```json
{
  "message": "Unsupported WASM version: 2. Expected version 1",
  "statusCode": 400
}
```

### Supported HTTP Status Codes

| Status | Meaning | Example |
|--------|---------|---------|
| 400 | Invalid WASM file | Malformed binary, invalid encoding |
| 401 | Unauthorized | User not authenticated |
| 413 | File too large | WASM exceeds size limits |
| 500 | Server error | Analysis failed, check logs |
| 503 | Service unavailable | Backend temporarily down |

## Customization

### Add Custom Error Patterns

Edit `lib/errorHandling.ts` to recognize custom error patterns:

```typescript
const wasmErrorPatterns = [
  {
    pattern: /your_custom_error|another_error/i,
    title: 'Custom Error Title',
    details: () => 'Detailed explanation of the error',
  },
  // ... more patterns
];
```

### Override Error Messages

Extend the error mapping in `parseWasmError()`:

```typescript
const defaultErrors: Record<number, WasmBackendError> = {
  419: {
    title: 'Custom Status Error',
    message: 'Your custom error message',
    statusCode: 419,
    suggestedAction: 'What the user should do',
  },
  // ... more status codes
};
```

## Testing Error Scenarios

### Test Invalid Base64

```bash
curl -X POST http://localhost:8080/analyze/wasm \
  -H "Content-Type: application/json" \
  -d '{"wasm_bytes": "invalid!!!base64", "function_name": "test", "args": []}'
```

### Test Malformed WASM

Create a minimal invalid WASM file and upload via the UI to see error handling.

### Test Timeout Scenarios

Set `RUST_LOG=debug` on backend to see detailed profiling logs:

```bash
RUST_LOG=debug cargo run -p soroscope-core
```

## UI States

The upload component now has these states:

| State | Description | Visual |
|-------|-------------|--------|
| `idle` | Waiting for file | Gray, normal |
| `hover` | User hovering with file | Blue highlight |
| `scanning` | Client-side validation in progress | Violet with animation |
| `submitting` | Sending to backend for validation | Violet, slower animation |
| `success` | File accepted by backend | Green checkmark |
| `error` | File rejected (client or server) | Red with details |

## Best Practices

1. **Always enable backend validation** in production to catch edge cases
2. **Provide clear file upload guidance** - mention "compiled Soroban contracts only"
3. **Monitor backend error logs** to identify common rejection patterns
4. **Update error patterns** as you discover new failure modes
5. **Test with various contract sizes** to identify resource limits early
6. **Display error details to advanced users** - collapsed by default, expandable

## Troubleshooting

### "Invalid base64 WASM data" Error

**Cause**: File reading or encoding failed  
**Fix**: Ensure the file is a valid compiled .wasm binary, not text

### "Unsupported WASM version" Error

**Cause**: Contract compiled with incompatible Soroban version  
**Fix**: Recompile using the matching Soroban CLI version

### Backend Returns 500 Error

**Cause**: Analysis or validation logic failed  
**Fix**: Check backend logs with `RUST_LOG=info cargo run -p soroscope-core`

### Upload Always Succeeds (No Backend Validation)

**Cause**: `enableBackendValidation={false}` is set  
**Fix**: Set to `true` or remove the prop (defaults to `true`)

## Future Enhancements

- [ ] Retry with exponential backoff for transient failures
- [ ] Webhook notifications for backend validation status
- [ ] Caching of validated WASM files
- [ ] Batch upload support with detailed per-file error reporting
- [ ] Integration with contract debugging tools
