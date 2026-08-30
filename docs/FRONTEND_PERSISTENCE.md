# Frontend State Persistence

This document describes how the SoroScope frontend persists WASM analysis results across page refreshes using browser local storage.

## Overview

The frontend now automatically saves the most recent WASM analysis result to browser local storage and restores it when the page reloads. This ensures users don't lose their latest analysis data when refreshing the page or navigating away and back.

## What Gets Persisted

The following data is saved for the latest analysis:

- **Analysis Result**: The complete `InvocationResult` object, including:
  - Function name and inputs
  - Success/error status
  - Resource cost metrics (CPU instructions, RAM, ledger I/O, transaction size)
  - Call graph data (if available)
  - State snapshots (if available)
  - Timestamp and unique ID

## How It Works

### Storage Key

The latest analysis is stored under the key: `soroscope-latest-analysis`

### Save Behavior

- **When**: Analysis results are saved immediately after completion (both success and error cases)
- **What**: Only the most recent result is kept - new analyses overwrite previous ones
- **Where**: Browser local storage (client-side only)

### Restore Behavior

- **When**: On initial page load/hydration
- **How**: The stored result is loaded into the UI state automatically
- **Fallback**: If no valid data exists or parsing fails, the UI shows the default empty state

### Browser-Only Execution

All storage operations are guarded to only run in browser environments. This prevents:
- Server-side rendering (SSR) errors in Next.js
- Hydration mismatches
- `localStorage is not defined` errors

## Implementation Details

### Core Module: `analysisStorage.ts`

Located at `web/lib/analysisStorage.ts`, this module provides three functions:

```typescript
// Save the latest analysis result
saveLatestAnalysis(result: InvocationResult): void

// Load the latest analysis result (returns null if unavailable)
loadLatestAnalysis(): InvocationResult | null

// Clear the stored result
clearLatestAnalysis(): void
```

### Integration Points

**In `web/pages/index.tsx`:**

1. **On mount**: Restore the latest analysis using `useEffect`
   ```typescript
   useEffect(() => {
     const restored = loadLatestAnalysis();
     if (restored) {
       setCurrentResult(restored);
     }
   }, []);
   ```

2. **After successful analysis**: Save the result
   ```typescript
   setCurrentResult(result);
   addToHistory(result);
   saveLatestAnalysis(result); // Persist to storage
   ```

3. **After error**: Save the error result
   ```typescript
   setCurrentResult(errorResult);
   addToHistory(errorResult);
   saveLatestAnalysis(errorResult); // Persist errors too
   ```

## Error Handling

The implementation handles several edge cases gracefully:

### Invalid or Malformed Data

If the stored data is corrupted or doesn't match the expected schema:
- A warning is logged to the console
- `null` is returned
- The UI falls back to the default empty state

### Storage Quota Exceeded

If local storage is full:
- The save operation fails silently
- A warning is logged to the console
- The app continues to function normally

### Missing localStorage API

If `localStorage` is unavailable (e.g., in private browsing mode or SSR):
- All operations become no-ops
- No errors are thrown
- The app works without persistence

## Limitations

### Browser-Only Storage

- Data is stored locally in the browser only
- Not synchronized across devices or browsers
- Cleared when browser data is cleared

### Single Result Only

- Only the most recent analysis is persisted
- Previous results are not kept (use the History tab for that)
- Each new analysis overwrites the previous one

### No Schema Versioning

- If the `InvocationResult` type changes significantly, old stored data may become incompatible
- Consider adding version metadata if breaking changes are planned

## Relationship to History

The persistence feature complements but does not replace the existing history feature:

| Feature | Latest Analysis Persistence | Invocation History |
|---------|----------------------------|-------------------|
| **Storage Key** | `soroscope-latest-analysis` | `soroban-invocation-history` |
| **Capacity** | 1 result (latest only) | Up to 10 results |
| **Purpose** | Restore UI state on refresh | Track multiple past invocations |
| **UI Location** | Result tab (auto-restored) | History tab (user-selected) |

Both features use local storage and follow the same browser-only execution pattern.

## Testing

To verify the persistence feature works correctly:

1. **Basic Persistence**
   - Run a WASM analysis
   - Refresh the page
   - Verify the result is still displayed

2. **Error Persistence**
   - Trigger an analysis error (e.g., invalid contract ID)
   - Refresh the page
   - Verify the error is still displayed

3. **Overwrite Behavior**
   - Run analysis A
   - Run analysis B
   - Refresh the page
   - Verify only analysis B is shown (A was overwritten)

4. **Invalid Storage**
   - Open browser DevTools → Application → Local Storage
   - Manually corrupt the `soroscope-latest-analysis` value
   - Refresh the page
   - Verify the app doesn't crash and shows empty state

5. **Storage Cleared**
   - Run an analysis
   - Clear browser local storage
   - Refresh the page
   - Verify the app shows empty state without errors

## Future Enhancements

Potential improvements for future iterations:

- **Schema versioning**: Add a version field to handle breaking changes gracefully
- **Compression**: Compress large results before storing to save space
- **Selective persistence**: Allow users to opt out of persistence
- **Cloud sync**: Optionally sync results across devices (requires backend)
- **Export/import**: Allow users to export and import analysis results

## Related Files

- `web/lib/analysisStorage.ts` - Core persistence logic
- `web/pages/index.tsx` - Integration with main UI
- `web/lib/sorobantypes.ts` - Type definitions for `InvocationResult`
- `web/components/InnovocationHistory.tsx` - Related history persistence feature
