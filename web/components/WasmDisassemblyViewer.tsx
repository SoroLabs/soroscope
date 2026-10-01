import { useEffect, useMemo, useRef, useState, type ChangeEvent } from "react";
import { Binary, FileCode, Search, Upload } from "lucide-react";

import {
  buildSourceLineMap,
  disassembleWasm,
  filterInstructions,
  type WasmDisassemblyResult,
  type WasmInstruction,
} from "../lib/wasmDisassembly";

/** Rows rendered at once; large contracts stay responsive and the filter narrows the rest. */
const MAX_VISIBLE_ROWS = 2000;
const ALL_FUNCTIONS = -1;

export interface WasmDisassemblyViewerProps {
  /** WASM uploaded elsewhere on the page. The viewer also accepts its own upload. */
  wasmFile?: File | null;
  /** Initial Rust source for the left pane; the user can paste or load their own. */
  rustSource?: string;
}

interface DisplayRow {
  instruction: WasmInstruction;
  depth: number;
}

/** Nesting depth per instruction so block/loop/if bodies read as indented WAT. */
function withDepth(instructions: WasmInstruction[]): DisplayRow[] {
  let depth = 0;
  let funcIndex = -1;
  return instructions.map((instruction) => {
    if (instruction.funcIndex !== funcIndex) {
      funcIndex = instruction.funcIndex;
      depth = 0;
    }
    const { mnemonic } = instruction;
    if (mnemonic === "end") depth = Math.max(0, depth - 1);
    const rowDepth = mnemonic === "else" ? Math.max(0, depth - 1) : depth;
    if (mnemonic === "block" || mnemonic === "loop" || mnemonic === "if") depth += 1;
    return { instruction, depth: rowDepth };
  });
}

export function WasmDisassemblyViewer({ wasmFile = null, rustSource = "" }: WasmDisassemblyViewerProps) {
  const [localFile, setLocalFile] = useState<File | null>(null);
  const [result, setResult] = useState<WasmDisassemblyResult | null>(null);
  const [decoding, setDecoding] = useState(false);
  const [source, setSource] = useState(rustSource);
  const [editingSource, setEditingSource] = useState(rustSource.length === 0);
  const [query, setQuery] = useState("");
  const [selectedFunction, setSelectedFunction] = useState(ALL_FUNCTIONS);
  const [selectedLine, setSelectedLine] = useState<number | null>(null);
  const firstHighlightRef = useRef<HTMLLIElement | null>(null);

  const file = localFile ?? wasmFile;

  // A new upload from the page replaces a file picked inside the viewer.
  useEffect(() => {
    if (wasmFile) setLocalFile(null);
  }, [wasmFile]);

  useEffect(() => {
    if (!file) {
      setResult(null);
      return;
    }
    let cancelled = false;
    setDecoding(true);
    file
      .arrayBuffer()
      .then((buffer) => {
        if (!cancelled) setResult(disassembleWasm(buffer));
      })
      .catch((error: unknown) => {
        if (!cancelled) {
          setResult({
            valid: false,
            byteLength: 0,
            sections: [],
            imports: [],
            exports: [],
            functions: [],
            instructions: [],
            errors: [error instanceof Error ? error.message : "Failed to read file"],
          });
        }
      })
      .finally(() => {
        if (!cancelled) setDecoding(false);
      });
    setSelectedFunction(ALL_FUNCTIONS);
    setSelectedLine(null);
    return () => {
      cancelled = true;
    };
  }, [file]);

  const functions = useMemo(() => result?.functions ?? [], [result]);
  const sourceLines = useMemo(() => source.split(/\r?\n/), [source]);
  const { lineMap, matchedFunctions } = useMemo(
    () => buildSourceLineMap(source, functions),
    [source, functions],
  );

  const highlighted = useMemo(
    () => new Set(selectedLine !== null ? lineMap[selectedLine] ?? [] : []),
    [lineMap, selectedLine],
  );

  const rows = useMemo(() => {
    const scoped =
      selectedFunction === ALL_FUNCTIONS
        ? result?.instructions ?? []
        : functions.find((fn) => fn.index === selectedFunction)?.instructions ?? [];
    // Depth is computed before filtering so matches keep their real indentation.
    const indented = withDepth(scoped);
    if (!query.trim()) return indented;
    const matches = new Set(filterInstructions(scoped, query));
    return indented.filter((row) => matches.has(row.instruction));
  }, [result, functions, selectedFunction, query]);

  const visibleRows = rows.slice(0, MAX_VISIBLE_ROWS);
  const firstHighlightId = visibleRows.find((row) => highlighted.has(row.instruction.id))?.instruction.id;

  const handleRustLineClick = (line: number) => {
    if (selectedLine === line) {
      setSelectedLine(null);
      return;
    }
    setSelectedLine(line);
    const ids = lineMap[line];
    if (!ids || ids.length === 0) return;
    // Jump the right pane to the compiled function and clear a filter that would hide it.
    const funcIndex = Number(ids[0].split(":")[0]);
    setSelectedFunction(funcIndex);
    setQuery("");
  };

  useEffect(() => {
    if (firstHighlightId) {
      firstHighlightRef.current?.scrollIntoView({ block: "nearest", behavior: "smooth" });
    }
  }, [firstHighlightId]);

  const handleWasmPick = (event: ChangeEvent<HTMLInputElement>) => {
    const picked = event.target.files?.[0];
    if (picked) setLocalFile(picked);
    event.target.value = "";
  };

  const handleSourcePick = async (event: ChangeEvent<HTMLInputElement>) => {
    const picked = event.target.files?.[0];
    event.target.value = "";
    if (!picked) return;
    setSource(await picked.text());
    setEditingSource(false);
    setSelectedLine(null);
  };

  return (
    <section aria-labelledby="disassembly-heading" className="space-y-4">
      <header className="flex flex-wrap items-center justify-between gap-3">
        <div className="flex items-center gap-3">
          <div className="flex h-8 w-8 items-center justify-center rounded-lg bg-slate-800">
            <Binary className="h-4 w-4 text-cyan-400" />
          </div>
          <div>
            <h2 id="disassembly-heading" className="text-sm font-semibold text-slate-200">
              WASM Disassembly
            </h2>
            <p className="text-xs text-slate-500">
              {file
                ? `${file.name} · ${functions.length} functions · ${result?.instructions.length ?? 0} opcodes`
                : "Upload a compiled contract to decode its bytecode"}
            </p>
          </div>
        </div>
        <label className="flex cursor-pointer items-center gap-2 rounded-lg border border-slate-700 px-3 py-1.5 text-xs font-medium text-slate-300 hover:bg-slate-800">
          <Upload className="h-3.5 w-3.5" />
          {file ? "Replace .wasm" : "Upload .wasm"}
          <input
            type="file"
            accept=".wasm,application/wasm"
            className="sr-only"
            onChange={handleWasmPick}
            data-testid="disassembly-wasm-input"
          />
        </label>
      </header>

      {result && result.errors.length > 0 && (
        <ul role="alert" className="space-y-1 rounded-lg border border-amber-700/50 bg-amber-950/30 p-3 text-xs text-amber-200">
          {result.errors.map((error) => (
            <li key={error}>{error}</li>
          ))}
        </ul>
      )}

      <div className="grid grid-cols-1 gap-4 lg:grid-cols-2">
        {/* Rust source */}
        <div className="flex min-w-0 flex-col rounded-xl border border-slate-800 bg-slate-950">
          <div className="flex items-center justify-between gap-2 border-b border-slate-800 px-3 py-2">
            <span className="flex items-center gap-2 text-xs font-medium text-slate-300">
              <FileCode className="h-3.5 w-3.5 text-cyan-400" />
              Rust source
              {source && functions.length > 0 && (
                <span className="text-slate-500">· {matchedFunctions.length} fn mapped</span>
              )}
            </span>
            <div className="flex items-center gap-2">
              <label className="cursor-pointer text-xs text-cyan-400 hover:text-cyan-300">
                Load .rs
                <input type="file" accept=".rs,text/plain" className="sr-only" onChange={handleSourcePick} />
              </label>
              {source && (
                <button
                  type="button"
                  onClick={() => setEditingSource((value) => !value)}
                  className="text-xs text-slate-400 hover:text-slate-200"
                >
                  {editingSource ? "Done" : "Edit"}
                </button>
              )}
            </div>
          </div>

          {editingSource ? (
            <textarea
              value={source}
              onChange={(event) => {
                setSource(event.target.value);
                setSelectedLine(null);
              }}
              spellCheck={false}
              placeholder="Paste the contract's Rust source (lib.rs) to cross-highlight opcodes"
              aria-label="Rust source"
              className="h-[480px] w-full resize-none bg-transparent p-3 font-mono text-xs text-slate-200 placeholder:text-slate-600 focus:outline-none"
            />
          ) : (
            <ol className="h-[480px] overflow-auto py-2 font-mono text-xs" aria-label="Rust source lines">
              {sourceLines.map((text, index) => {
                const line = index + 1;
                const mapped = Boolean(lineMap[line]);
                const active = selectedLine === line;
                return (
                  <li key={line}>
                    <button
                      type="button"
                      onClick={() => handleRustLineClick(line)}
                      aria-pressed={active}
                      data-line={line}
                      className={`flex w-full gap-3 px-3 text-left whitespace-pre ${
                        active
                          ? "bg-cyan-500/20 text-cyan-100"
                          : mapped
                            ? "text-slate-200 hover:bg-slate-800/70"
                            : "text-slate-500 hover:bg-slate-900"
                      }`}
                    >
                      <span
                        className={`w-8 shrink-0 select-none text-right ${mapped ? "text-cyan-600" : "text-slate-700"}`}
                      >
                        {line}
                      </span>
                      <span>{text || " "}</span>
                    </button>
                  </li>
                );
              })}
            </ol>
          )}
        </div>

        {/* Disassembly */}
        <div className="flex min-w-0 flex-col rounded-xl border border-slate-800 bg-slate-950">
          <div className="flex flex-wrap items-center gap-2 border-b border-slate-800 px-3 py-2">
            <select
              value={selectedFunction}
              onChange={(event) => setSelectedFunction(Number(event.target.value))}
              aria-label="Function"
              className="min-w-0 max-w-[45%] rounded-md border border-slate-700 bg-slate-900 px-2 py-1 text-xs text-slate-200"
            >
              <option value={ALL_FUNCTIONS}>All functions</option>
              {functions.map((fn) => (
                <option key={fn.index} value={fn.index}>
                  {fn.name}
                </option>
              ))}
            </select>
            <div className="relative min-w-0 flex-1">
              <Search className="pointer-events-none absolute left-2 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-slate-500" />
              <input
                type="search"
                value={query}
                onChange={(event) => setQuery(event.target.value)}
                placeholder="Filter opcodes (br_if, call swap, 0x10)"
                aria-label="Filter opcodes"
                className="w-full rounded-md border border-slate-700 bg-slate-900 py-1 pl-7 pr-2 text-xs text-slate-200 placeholder:text-slate-600 focus:outline-none focus:ring-1 focus:ring-cyan-500/60"
              />
            </div>
          </div>

          <ol className="h-[480px] overflow-auto py-2 font-mono text-xs" aria-label="WASM opcodes">
            {decoding && <li className="px-3 text-slate-500">Decoding…</li>}
            {!decoding && !file && (
              <li className="px-3 py-8 text-center text-slate-500">No WASM loaded</li>
            )}
            {!decoding && file && rows.length === 0 && (
              <li className="px-3 py-8 text-center text-slate-500">
                {query ? `No opcodes match “${query}”` : "No instructions decoded"}
              </li>
            )}
            {visibleRows.map(({ instruction, depth }, position) => {
              const isHighlighted = highlighted.has(instruction.id);
              const startsFunction =
                selectedFunction === ALL_FUNCTIONS &&
                !query.trim() &&
                (position === 0 || visibleRows[position - 1].instruction.funcIndex !== instruction.funcIndex);
              return (
                <li
                  key={instruction.id}
                  ref={instruction.id === firstHighlightId ? firstHighlightRef : undefined}
                  data-id={instruction.id}
                  data-highlighted={isHighlighted || undefined}
                >
                  {startsFunction && (
                    <div className="mt-2 px-3 text-cyan-500">
                      ;; func[{instruction.funcIndex}] {instruction.funcName}
                    </div>
                  )}
                  <div
                    className={`flex gap-3 px-3 whitespace-pre ${
                      isHighlighted ? "bg-amber-400/15 text-amber-100" : "text-slate-300"
                    }`}
                  >
                    <span className="w-14 shrink-0 select-none text-right text-slate-600">
                      {instruction.offset.toString(16).padStart(6, "0")}
                    </span>
                    <span>
                      {"  ".repeat(depth)}
                      <span className={instruction.unknown ? "text-red-400" : "text-violet-300"}>
                        {instruction.mnemonic}
                      </span>
                      {instruction.operands.length > 0 && (
                        <span className="text-slate-400"> {instruction.operands.join(" ")}</span>
                      )}
                    </span>
                  </div>
                </li>
              );
            })}
            {rows.length > MAX_VISIBLE_ROWS && (
              <li className="px-3 py-2 text-slate-500">
                Showing {MAX_VISIBLE_ROWS} of {rows.length} opcodes — pick a function or filter to narrow.
              </li>
            )}
          </ol>
        </div>
      </div>

      {file && source && functions.length > 0 && matchedFunctions.length === 0 && (
        <p className="text-xs text-slate-500">
          No Rust functions matched WASM symbols. Cross-highlighting needs a build that keeps the{" "}
          <code className="text-slate-400">name</code> section or exports matching your <code className="text-slate-400">fn</code> names.
        </p>
      )}
    </section>
  );
}
