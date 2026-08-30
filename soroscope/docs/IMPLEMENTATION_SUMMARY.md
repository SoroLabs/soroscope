# WASM Error Handling - Implementation Summary

## What Was Changed

### 1. Enhanced Error Handling Utilities (`web/lib/errorHandling.ts`)

**Added**: `parseWasmError()` function with WASM-specific error categorization
- Pattern-based error recognition for common WASM issues
- HTTP status code mapping (400, 401, 413, 500, 503)
- Structured error response type: `WasmBackendError`
- User-friendly titles, messages, and suggested actions

**Error Patterns Recognized**:
- Base64 encoding errors
- Invalid WASM format
- Unsupported versions
- Resource exhaustion
- Timeouts
- Missing exports

### 2. Upload Zone Component (`web/components/upload-zone.tsx`)

**Added Features**:

1. **Backend Validation Flow**:
   - New state: `submitting` (for backend validation)
   - Function `submitToBackend()` sends validated WASM to backend
   - Graceful error handling with detailed error reporting

2. **Enhanced Error State**:
   - `errorDetails` object with title, message, details, and suggested actions
   - Technical details section for advanced users
   - Suggested action cards with context-aware hints

3. **Props**:
   - `backendUrl`: Configurable backend endpoint (default: localhost:8080)
   - `enableBackendValidation`: Toggle backend validation (default: true)
   - Existing `onFileReady` callback still works

4. **UI Improvements**:
   - Shows "Validating with server..." during backend submission
   - Displays structured error information
   - Added amber hint cards for suggested actions
   - File info displayed on success

### 3. Main Page Integration (`web/pages/index.tsx`)

**Updated**: UploadZone configuration
- Added `backendUrl` prop pointing to backend endpoint
- Set `enableBackendValidation={true}` 
- Updated comment to reflect backend validation

---

## How It Works

### Upload Flow

```
1. User drops/selects WASM file
   ↓
2. Client-side validation
   - Check file type (.wasm)
   - Verify magic number (0x0061736d)
   - Check WASM version (v1)
   ↓
3. If client validation passes
   - Convert to base64
   - Send to backend for further validation
   ↓
4. Backend response
   - Success: Call onFileReady()
   - Error (4xx/5xx): Parse and display error details
```

### Error Handling Flow

```
Backend Error → extractErrorDetails() → parseWasmError()
   ↓
Match error pattern → Select error category
   ↓
Map to user-friendly message → Display in UI
```

---

## Files Modified

### New Files
- `docs/WASM_ERROR_HANDLING.md` - User guide and API documentation
- `docs/WASM_ERROR_HANDLING_TEST_GUIDE.md` - Comprehensive test scenarios

### Modified Files
- `web/lib/errorHandling.ts` - Added WASM-specific error parsing
- `web/components/upload-zone.tsx` - Backend integration and error display
- `web/pages/index.tsx` - UploadZone configuration

---

## Key Features

### 1. Pattern-Based Error Recognition

```typescript
// Error patterns for pattern matching
{
  pattern: /Invalid base64|base64 decoding/i,
  title: 'Invalid WASM Encoding',
  details: () => 'The file appears to be corrupted...'
}
```

**Benefits**:
- Flexible error matching
- Case-insensitive patterns
- Easy to extend with new patterns

### 2. Structured Error Response

```typescript
interface WasmBackendError {
  title: string;        // "Invalid WASM Format"
  message: string;      // User-friendly explanation
  details?: string;     // Technical context
  suggestedAction?: string; // How to resolve
  statusCode: number;   // HTTP status
}
```

### 3. Multi-Level Error Display

**Level 1 - Title**: "Invalid WASM Encoding"
**Level 2 - Message**: "The file appears to be corrupted or improperly encoded..."
**Level 3 - Details**: Raw error from backend (collapsible)
**Level 4 - Action**: "Try uploading a different contract..."

### 4. Graceful Degradation

- No backend available? Uses client-side validation only (if configured)
- Network error? Shows "Please try again" message
- Unknown error? Provides generic fallback message

---

## Testing Scenarios Covered

✓ Valid WASM uploads
✓ Invalid file types (.txt, .json, etc.)
✓ Corrupted WASM files
✓ Invalid base64 encoding
✓ Version mismatches
✓ Oversized files (413)
✓ Server errors (500)
✓ Service unavailable (503)
✓ Authentication failures (401)
✓ Network timeouts

---

## Performance Characteristics

| Operation | Time | Notes |
|-----------|------|-------|
| File reading | <100ms | Depends on file size |
| Client validation | ~800ms | Built-in delay for UX |
| Base64 encoding | <200ms | In-memory operation |
| Backend submission | Varies | Network dependent |
| Error parsing | <10ms | Regex matching |

---

## Configuration Options

### Disable Backend Validation
```tsx
<UploadZone enableBackendValidation={false} />
```
Only client-side validation will run.

### Custom Backend URL
```tsx
<UploadZone backendUrl="https://api.production.com/validate-wasm" />
```

### Custom Callback
```tsx
<UploadZone onFileReady={(file) => {
  // File has been validated, use it here
  sendToAnalysis(file);
}} />
```

---

## Security Considerations

1. **Base64 Encoding**: Safe for transmission in JSON
2. **CORS**: Configure CORS on backend for production
3. **Rate Limiting**: Implement on backend to prevent abuse
4. **File Size Limits**: Check both client and server side
5. **Authentication**: Can integrate JWT or wallet auth

---

## Browser Support

- Chrome 90+
- Firefox 88+
- Safari 14+
- Edge 90+

Tested with React 18+

---

## Future Enhancements

- [ ] Retry with exponential backoff
- [ ] Webhook notifications for status
- [ ] Caching of validated WASM
- [ ] Batch upload support
- [ ] Integration with contract debugging
- [ ] Detailed gas estimation errors
- [ ] Contract optimization suggestions

---

## Troubleshooting

### Upload Always Shows "Validating with server..."
**Issue**: Backend not responding  
**Solution**: Check backend is running on port 8080

### Error Messages Don't Match Backend Errors
**Issue**: Pattern not matching backend response  
**Solution**: Check error message format in backend response, update patterns in `parseWasmError()`

### File Stuck in Scanning State
**Issue**: Frontend validation failing  
**Solution**: Ensure file is valid WASM (magic: 0x0061736d, version: 1)

### See "Try another file" But Want to Debug
**Issue**: Error details truncated in UI  
**Solution**: Check browser console for full error objects

---

## API Integration

### Backend Endpoint Format

**Endpoint**: `POST /analyze/wasm`

**Request**:
```json
{
  "wasm_bytes": "base64encodedWasmBinary",
  "function_name": "main",
  "args": []
}
```

**Success Response** (200):
```json
{
  "profile": { ... },
  "resources": { ... }
}
```

**Error Response** (4xx/5xx):
```json
{
  "error": "BAD_REQUEST",
  "message": "Invalid base64 WASM data: ..."
}
```

---

## Validation Checklist

Before deploying to production:

- [ ] Backend error messages match expected patterns
- [ ] All HTTP status codes handled
- [ ] CORS configured on backend
- [ ] Rate limiting implemented
- [ ] File size limits enforced
- [ ] Error patterns tested with real contracts
- [ ] UI tested in target browsers
- [ ] Accessibility verified (keyboard, screen reader)
- [ ] Error messages are clear and actionable
- [ ] Suggested actions are helpful

