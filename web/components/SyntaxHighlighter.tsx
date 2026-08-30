"use client";

import React, { useMemo } from "react";
import { CopyButton } from "./CopyButton";

export type HighlightLanguage = "contract" | "xdr";

interface SyntaxHighlighterProps {
  /** The source code or XDR string to highlight */
  code: string;
  /** Language mode for tokenization rules */
  language: HighlightLanguage;
  /** Whether to show line numbers (default: false) */
  showLineNumbers?: boolean;
  /** Optional CSS class name for the wrapper */
  className?: string;
  /** Max height for scrollable area (default: 'auto') */
  maxHeight?: string;
}

// ──────────────────────────────────────────────
// Tokenization Rules
// ──────────────────────────────────────────────

interface TokenRule {
  pattern: RegExp;
  type: string; // maps to a CSS class suffix
}

const CONTRACT_KEYWORDS = [
  "pub", "fn", "let", "mut", "const", "if", "else", "for", "while",
  "loop", "match", "return", "true", "false", "Self", "self",
  "struct", "enum", "impl", "use", "mod", "type", "trait", "where",
  "as", "in", "ref", "move", "async", "await", "unsafe",
  "import", "contract", "interface", "export",
];

const CONTRACT_TYPES = [
  "u32", "u64", "u128", "i32", "i64", "i128", "bool", "String",
  "Address", "Symbol", "Bytes", "BytesN", "Vec", "Map", "Option",
  "Result", "SorobanType", "IntoVal", "TryFromVal", "Env",
  "BigInt", "Duration", "Timepoint",
];

const CONTRACT_TOKEN_RULES: TokenRule[] = [
  // Strings (double or single quoted)
  { pattern: /"([^"\\]*(?:\\.[^"\\]*)*)"|'([^'\\]*(?:\\.[^'\\]*)*)'/y, type: "string" },
  // Comments (line and block)
  { pattern: /\/\/[^\n]*|\/\*[\s\S]*?\*\//y, type: "comment" },
  // Numbers (hex, float, int)
  { pattern: /0x[0-9a-fA-F_]+|\d+\.\d+|\d+_?\d*/y, type: "number" },
  // Keywords
  { pattern: new RegExp(`\\b(${CONTRACT_KEYWORDS.join("|")})\\b`, "y"), type: "keyword" },
  // Types
  { pattern: new RegExp(`\\b(${CONTRACT_TYPES.join("|")})\\b`, "y"), type: "type" },
  // Function calls / identifiers followed by (
  { pattern: /[a-zA-Z_]\w*(?=\s*\()/y, type: "function" },
  // Operators
  { pattern: /[{}()\[\];:.,=+\-*/!~<>%&|^?@]+/y, type: "operator" },
  // Identifiers / words
  { pattern: /[a-zA-Z_]\w*/y, type: "default" },
  // Whitespace
  { pattern: /\s+/y, type: "whitespace" },
  // Fallback: any single character
  { pattern: /./y, type: "default" },
];

const XDR_TOKEN_RULES: TokenRule[] = [
  // Hex strings (like contract IDs / hashes)
  { pattern: /[A-Fa-f0-9]{32,}/y, type: "hex" },
  // Base64-like strings (XDR payloads)
  { pattern: /[A-Za-z0-9+/=]{20,}/y, type: "base64" },
  // Strings (double quoted)
  { pattern: /"([^"\\]*(?:\\.[^"\\]*)*)"/y, type: "string" },
  // Numbers
  { pattern: /\d+/y, type: "number" },
  // Field names (alphanumeric with underscores, may be followed by :)
  { pattern: /[a-zA-Z_]\w*(?=\s*:)/y, type: "field" },
  // Keywords (XDR structural)
  { pattern: /\b(enum|struct|union|case|default|void|bool|int|unsigned|hyper|opaque|string|array|optional|switch|typedef|const)\b/y, type: "keyword" },
  // Punctuation / delimiters
  { pattern: /[{}()\[\]:;,.=<>*|]+/y, type: "punctuation" },
  // Identifiers
  { pattern: /[a-zA-Z_]\w*/y, type: "default" },
  // Whitespace
  { pattern: /\s+/y, type: "whitespace" },
  // Fallback
  { pattern: /./y, type: "default" },
];

// ──────────────────────────────────────────────
// Tokenizer
// ──────────────────────────────────────────────

interface Token {
  text: string;
  type: string;
}

function tokenize(code: string, rules: TokenRule[]): Token[] {
  const tokens: Token[] = [];
  let pos = 0;

  while (pos < code.length) {
    let matched = false;

    for (const rule of rules) {
      // Reset lastIndex for sticky regex
      rule.pattern.lastIndex = pos;
      const match = rule.pattern.exec(code);

      if (match && match.index === pos) {
        tokens.push({ text: match[0], type: rule.type });
        pos += match[0].length;
        matched = true;
        break;
      }
    }

    if (!matched) {
      // Shouldn't happen if fallback rule exists, but just in case
      tokens.push({ text: code[pos], type: "default" });
      pos++;
    }
  }

  return tokens;
}

// ──────────────────────────────────────────────
// Color map for token types
// ──────────────────────────────────────────────

const TOKEN_COLORS: Record<string, string> = {
  // Contract source
  keyword: "text-cyan-400",
  type: "text-yellow-400",
  string: "text-emerald-400",
  number: "text-orange-400",
  comment: "text-slate-500 italic",
  function: "text-violet-400",
  operator: "text-slate-300",
  whitespace: "",
  default: "text-slate-200",

  // XDR
  field: "text-blue-400",
  hex: "text-fuchsia-400",
  base64: "text-amber-300/80",
  punctuation: "text-slate-400",
};

function getTokenClass(type: string): string {
  return TOKEN_COLORS[type] || TOKEN_COLORS.default;
}

// ──────────────────────────────────────────────
// Component
// ──────────────────────────────────────────────

export function SyntaxHighlighter({
  code,
  language,
  showLineNumbers = false,
  className = "",
  maxHeight = "auto",
}: SyntaxHighlighterProps) {
  const tokens = useMemo(() => {
    const rules = language === "contract" ? CONTRACT_TOKEN_RULES : XDR_TOKEN_RULES;
    return tokenize(code, rules);
  }, [code, language]);

  // Group tokens into lines
  const lines = useMemo(() => {
    const result: Token[][] = [];
    let currentLine: Token[] = [];

    for (const token of tokens) {
      // Split tokens containing newlines
      const parts = token.text.split(/(\n)/);
      for (let i = 0; i < parts.length; i++) {
        if (parts[i] === "\n") {
          result.push(currentLine);
          currentLine = [];
        } else if (parts[i]) {
          currentLine.push({ text: parts[i], type: token.type });
        }
      }
    }

    if (currentLine.length > 0 || code.endsWith("\n")) {
      result.push(currentLine);
    }

    return result;
  }, [tokens, code]);

  const languageLabel =
    language === "contract" ? "Contract Source" : "XDR";

  return (
    <div
      className={`rounded-xl border border-slate-800 bg-slate-950 overflow-hidden ${className}`}
    >
      {/* Header bar */}
      <div className="flex items-center justify-between border-b border-slate-800 bg-slate-900/80 px-4 py-2">
        <div className="flex items-center gap-2">
          <span className="flex h-5 w-5 items-center justify-center rounded bg-slate-800 text-[10px] font-mono font-bold text-slate-400 uppercase tracking-wider">
            {language === "contract" ? "RS" : "XD"}
          </span>
          <span className="text-xs font-medium text-slate-400">{languageLabel}</span>
          <span className="text-[11px] text-slate-600 font-mono">
            {lines.length} lines
          </span>
        </div>
        <CopyButton
          text={code}
          label="Copy"
          iconSize={14}
          variant="ghost"
          tooltipPosition="left"
        />
      </div>

      {/* Code area */}
      <div
        className="overflow-auto"
        style={{ maxHeight }}
      >
        <table className="w-full border-collapse font-mono text-[13px] leading-relaxed">
          <tbody>
            {lines.map((line, lineIndex) => {
              const lineNumber = lineIndex + 1;
              return (
                <tr
                  key={lineIndex}
                  className="group hover:bg-slate-900/50 transition-colors"
                >
                  {showLineNumbers && (
                    <td className="select-none border-r border-slate-800/60 px-3 py-0 text-right text-[11px] text-slate-600 group-hover:text-slate-500 w-12 align-top">
                      {lineNumber}
                    </td>
                  )}
                  <td className={`px-4 py-0 ${showLineNumbers ? "" : "px-4"}`}>
                    {line.length === 0 ? (
                      <span className="text-slate-800"> </span>
                    ) : (
                      line.map((token, tokenIndex) => (
                        <span
                          key={tokenIndex}
                          className={getTokenClass(token.type)}
                        >
                          {token.text}
                        </span>
                      ))
                    )}
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>

        {code.length === 0 && (
          <div className="flex items-center justify-center py-12 text-sm text-slate-600">
            No source code to display
          </div>
        )}
      </div>
    </div>
  );
}

