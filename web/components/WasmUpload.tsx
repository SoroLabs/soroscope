"use client";

import React, { useCallback, useState } from "react";
import { useDropzone } from "react-dropzone";
import { motion, AnimatePresence } from "framer-motion";
import {
  Upload,
  FileCode,
  X,
  CheckCircle2,
  AlertCircle,
  Loader2,
  FileUp,
  ChevronRight,
} from "lucide-react";
import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";
import { ApiError, analyzeService } from "../lib/api";
import { createUserFriendlyMessage, formatError } from "../lib/errorHandling";
import { arrayBufferToBase64 } from "../lib/utils";
import { useWasmValidationWorker } from "../hooks/useWasmValidationWorker";
import { extractContractFunctions } from "../lib/wasmValidation";
import type { WasmValidationReport } from "../lib/wasmValidation";

// Utility for cleaner tailwind classes
function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

// Types

interface WasmFile {
  file: File;
  id: string;
  status: "pending" | "validating" | "uploading" | "success" | "error";
  progress: number;
  error?: string;
  hash?: string;
  simulationResult?: unknown;
  /** Structured decode report produced off the main thread. */
  validation?: WasmValidationReport;
}

// Maximum allowed WASM file size (2 MB) — enforced on both client and server.
const MAX_WASM_SIZE = 2 * 1024 * 1024; // 2 MB

// WASM magic-number: the four bytes \0asm (0x00 0x61 0x73 0x6D).
const WASM_MAGIC = 0x0061736d;

interface WasmUploadProps {
  onUploadComplete?: (files: WasmFile[]) => void;
  onFileSelect?: (files: File[]) => void;
  maxFileSize?: number; // in bytes, default 2 MB
  maxFiles?: number;
  className?: string;
}

// Helpers moved outside component to prevent re-creation on every render
const statusToErrorType = (status: number): string => {
  switch (status) {
    case 400:
      return "BAD_REQUEST";
    case 401:
      return "UNAUTHORIZED";
    case 404:
      return "NOT_FOUND";
    case 500:
      return "INTERNAL_SERVER_ERROR";
    case 503:
      return "SERVICE_UNAVAILABLE";
    default:
      return "UNKNOWN_ERROR";
  }
};

// Generate file hash (SHA-256) for WASM identification
const generateHash = async (file: File): Promise<string> => {
  const buffer = await file.arrayBuffer();
  const hashBuffer = await crypto.subtle.digest("SHA-256", buffer);
  const hashArray = Array.from(new Uint8Array(hashBuffer));
  return hashArray.map((b) => b.toString(16).padStart(2, "0")).join("");
};

export default function WasmUpload({
  onUploadComplete,
  onFileSelect,
  maxFileSize = MAX_WASM_SIZE,
  maxFiles = 5,
  className,
}: WasmUploadProps) {
  const [files, setFiles] = useState<WasmFile[]>([]);
  const [isDragActive, setIsDragActive] = useState(false);
  const { validate } = useWasmValidationWorker();

  // Validate WASM file
  const validateWasm = useCallback(
    (file: File): string | null => {
      if (!file.name.toLowerCase().endsWith(".wasm")) {
        return "Validation Error: File must be a .wasm file";
      }
      if (file.size > maxFileSize) {
        return `File too large (max ${(maxFileSize / 1024 / 1024).toFixed(1)}MB)`;
      if (file.size === 0) {
        return "File is empty";
      return null;
    },
    [maxFileSize]
  );
  //validate WASM file (synchronous checks: extension, size, empty)
  const validateWasm = useCallback((file: File): string | null => {
      return `File too large: ${(file.size / 1024 / 1024).toFixed(2)} MB exceeds the ${(maxFileSize / 1024 / 1024).toFixed(0)} MB limit`;
  }, [maxFileSize]);

  // Async magic-bytes check: reads the first 8 bytes and verifies \0asm + version 1.
  const validateWasmMagic = useCallback(async (file: File): Promise<string | null> => {
    try {
      const headerSlice = file.slice(0, 8);
      const buffer = await headerSlice.arrayBuffer();
      if (buffer.byteLength < 8) {
        return "File is too small to be a valid WebAssembly module";
      }
      const view = new DataView(buffer);
      const magic = view.getUint32(0, false); // big-endian: \0asm
      if (magic !== WASM_MAGIC) {
        return "Invalid WASM magic bytes — file does not start with \\0asm. Ensure you uploaded a compiled Soroban contract.";
      }
      const version = view.getUint32(4, true); // little-endian version field
      if (version !== 1) {
        return `Unsupported WASM version: ${version}. Expected version 1.`;
      }
      return null;
    } catch {
      return "Could not read file header for WASM validation";
    }
  }, []);

  const setUploadProgress = useCallback((id: string, progress: number) => {
    setFiles((prev) =>
      prev.map((f) => (f.id === id ? { ...f, progress } : f))
    );
  }, []);

  // Submit WASM to backend simulation engine.
  const uploadFile = useCallback(async (wasmFile: WasmFile) => {
    setFiles((prev) =>
      prev.map((f) =>
        f.id === wasmFile.id ? { ...f, status: "uploading" } : f
      )
    );

    try {
      setUploadProgress(wasmFile.id, 10);
      const buffer = await wasmFile.file.arrayBuffer();

      // Decode/validate in the Web Worker first. The worker takes ownership of
      // `buffer` (transferable), so base64 encoding uses its own copy.
      const encodeBuffer = buffer.slice(0);
        prev.map((f) => (f.id === wasmFile.id ? { ...f, status: "validating" } : f))
      const validation = await validate(buffer, maxFileSize);
      setUploadProgress(wasmFile.id, 40);

      if (!validation.valid) {
        throw new Error(
          validation.errors[0] || "Validation Error: WASM module could not be decoded"
      }

          f.id === wasmFile.id ? { ...f, status: "uploading", validation } : f

      const wasmBytesBase64 = arrayBufferToBase64(encodeBuffer);
      setUploadProgress(wasmFile.id, 80);

      const simulationResult = await analyzeService.analyzeWasm({
        wasm_bytes: wasmBytesBase64,
        function_name: "main",
        args: [],
      });

      const hash = await generateHash(wasmFile.file);
      setUploadProgress(wasmFile.id, 100);

  // Submit WASM to backend simulation engine
  const uploadFile = useCallback(
    async (wasmFile: WasmFile) => {
      setFiles((prev) =>
        prev.map((f) =>
          f.id === wasmFile.id ? { ...f, status: "uploading" } : f
        )
      );

      try {
        setUploadProgress(wasmFile.id, 20);
        const buffer = await wasmFile.file.arrayBuffer();
        setUploadProgress(wasmFile.id, 50);

        const wasmBytesBase64 = arrayBufferToBase64(buffer);
        setUploadProgress(wasmFile.id, 80);

        const simulationResult = await analyzeService.analyzeWasm({
          wasm_bytes: wasmBytesBase64,
          function_name: "main",
          args: [],
        });

        const hash = await generateHash(wasmFile.file);
        setUploadProgress(wasmFile.id, 100);

        setFiles((prev) =>
          prev.map((f) =>
            f.id === wasmFile.id
              ? { ...f, status: "success", progress: 100, hash, simulationResult }
              : f
          )
        );
      } catch (err) {
        let errorMessage = "Upload failed. Please try again.";

        if (err instanceof ApiError) {
          const body =
            typeof err.body === "object" && err.body !== null
              ? (err.body as { error?: unknown; message?: unknown })
              : undefined;
          const backendError = {
            error:
              typeof body?.error === "string"
                ? body.error
                : statusToErrorType(err.status),
            message:
              typeof body?.message === "string" ? body.message : err.message,
            statusCode: err.status,
          };
          errorMessage = createUserFriendlyMessage(backendError);
        } else {
          const formatted = formatError(err);
          errorMessage = formatted.message;
        }

        setFiles((prev) =>
          prev.map((f) =>
            f.id === wasmFile.id
              ? {
                  ...f,
                  status: "error",
                  error: errorMessage,
                }
              : f
          )
        );
      }

      setFiles((prev) =>
        prev.map((f) =>
          f.id === wasmFile.id
            ? {
                ...f,
                status: "error",
                error: errorMessage,
              }
            : f
        )
      );
  }, [maxFileSize, setUploadProgress, validate]);
    },
    [setUploadProgress]

  const onDrop = useCallback(
    async (acceptedFiles: File[]) => {
      const newFiles: WasmFile[] = acceptedFiles.map((file) => ({
        file,
        id: `${file.name}-${Date.now()}-${Math.random().toString(36).slice(2)}`,
        status: "pending",
        progress: 0,
      }));

      //synchronous checks: extension, size, empty
      const validFiles: WasmFile[] = [];
      const invalidFiles: WasmFile[] = [];

      newFiles.forEach((wasmFile) => {
        const error = validateWasm(wasmFile.file);
        if (error) {
          invalidFiles.push({ ...wasmFile, status: "error", error });
        } else {
          validFiles.push(wasmFile);
        }
      });

      // Async magic-bytes check — runs BEFORE upload dispatch
      const magicValidFiles: WasmFile[] = [];
      for (const wasmFile of validFiles) {
        const magicError = await validateWasmMagic(wasmFile.file);
        if (magicError) {
          invalidFiles.push({ ...wasmFile, status: "error", error: magicError });
        } else {
          magicValidFiles.push(wasmFile);
        }
      }

      if (invalidFiles.length > 0) {
        alert(
          invalidFiles[0].error ||
            "Validation Error: Invalid non-WASM file uploaded."
        );
      }

      const totalFiles = [...files, ...magicValidFiles, ...invalidFiles];
      if (totalFiles.length > maxFiles) {
        alert(`Maximum ${maxFiles} files allowed`);
        return;
      }

      setFiles((prev) => [...prev, ...magicValidFiles, ...invalidFiles]);
      onFileSelect?.(magicValidFiles.map((f) => f.file));

      validFiles.forEach((f) => uploadFile(f));
      //auto-upload valid files (after magic-bytes check passes)
      magicValidFiles.forEach((f) => uploadFile(f));
    },
    [files, maxFiles, onFileSelect, uploadFile, validateWasm, validateWasmMagic]
  );

  const { getRootProps, getInputProps, isDragReject } = useDropzone({
    onDrop,
    accept: {
      "application/wasm": [".wasm"],
    },
    maxFiles,
    maxSize: maxFileSize,
    onDragEnter: () => setIsDragActive(true),
    onDragLeave: () => setIsDragActive(false),
    onDropAccepted: () => setIsDragActive(false),
    onDropRejected: () => setIsDragActive(false),
  });

  const removeFile = (id: string) => {
    setFiles((prev) => prev.filter((f) => f.id !== id));
  };

  const clearAll = () => {
    setFiles([]);
  };

  const retryUpload = (id: string) => {
    const file = files.find((f) => f.id === id);
    if (file && file.status === "error") {
      setFiles((prev) =>
        prev.map((f) =>
          f.id === id
            ? { ...f, status: "pending", error: undefined, progress: 0 }
            : f
        )
      );
      uploadFile({ ...file, status: "pending", error: undefined, progress: 0 });
    }
  };

  const uploadingCount = files.filter(
    (f) => f.status === "uploading" || f.status === "validating"
  ).length;
  const uploadingCount = files.filter((f) => f.status === "uploading").length;
  const successCount = files.filter((f) => f.status === "success").length;

  const rootProps = getRootProps() as Omit<
    ReturnType<typeof getRootProps>,
    "onAnimationStart" | "onAnimationEnd" | "onAnimationIteration"
  >;

  return (
    <div className={cn("w-full max-w-2xl mx-auto", className)}>
      {/* Drop Zone */}
      <motion.div
        {...rootProps}
        className={cn(
          "relative border-2 border-dashed rounded-2xl p-8 text-center cursor-pointer transition-colors duration-200",
          isDragActive
            ? "border-indigo-500 bg-indigo-50/50"
            : isDragReject
            ? "border-red-400 bg-red-50/50"
            : "border-slate-300 hover:border-slate-400 bg-slate-50/50 hover:bg-slate-50"
        )}
      >
        <input {...getInputProps()} />

        <motion.div
          animate={isDragActive ? { y: [0, -8, 0] } : { y: 0 }}
          transition={{ repeat: isDragActive ? Infinity : 0, duration: 1.5 }}
        >
          <div
            className={cn(
              "mx-auto w-16 h-16 rounded-2xl flex items-center justify-center mb-4",
              isDragActive
                ? "bg-indigo-100 text-indigo-600"
                : "bg-slate-100 text-slate-400"
            )}
          >
            <Upload className="w-8 h-8" />
          </div>
        </motion.div>

        <h3 className="text-lg font-semibold text-slate-800 mb-1">
          {isDragActive
            ? "Drop your WASM files here"
            : "Upload Soroban WASM Contracts"}
        </h3>
        <p className="text-sm text-slate-500 mb-4">
          Drag & drop{" "}
          <code className="px-1.5 py-0.5 bg-slate-200 rounded text-xs font-mono">
            .wasm
          </code>{" "}
          files, or click to browse
        </p>
        <div className="flex items-center justify-center gap-2 text-xs text-slate-400">
          <FileCode className="w-4 h-4" />
          <span>Max {(maxFileSize / 1024 / 1024).toFixed(0)}MB per file</span>
          <span className="text-slate-300">•</span>
          <span>Up to {maxFiles} files</span>
        </div>
      </motion.div>

      {/* File List */}
      <AnimatePresence>
        {files.length > 0 && (
          <motion.div
            initial={{ opacity: 0, y: 20 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: -20 }}
            className="mt-6 space-y-3"
          >
            {/* Header */}
            <div className="flex items-center justify-between px-1">
              <div className="flex items-center gap-2">
                <span className="text-sm font-medium text-slate-700">
                  {files.length} file{files.length !== 1 ? "s" : ""}
                </span>
                {uploadingCount > 0 && (
                  <span className="text-xs text-indigo-600 bg-indigo-50 px-2 py-0.5 rounded-full">
                    {uploadingCount} uploading
                  </span>
                )}
                {successCount > 0 && (
                  <span className="text-xs text-emerald-600 bg-emerald-50 px-2 py-0.5 rounded-full">
                    {successCount} ready
                  </span>
                )}
              </div>
              <button
                onClick={clearAll}
                className="text-xs text-slate-400 hover:text-red-500 transition-colors"
              >
                Clear all
              </button>
            </div>

            {/* File Items */}
            {files.map((wasmFile) => (
              <motion.div
                key={wasmFile.id}
                layout
                initial={{ opacity: 0, scale: 0.95 }}
                animate={{ opacity: 1, scale: 1 }}
                exit={{ opacity: 0, scale: 0.95 }}
                className={cn(
                  "relative bg-white border rounded-xl p-4 shadow-sm",
                  wasmFile.status === "error"
                    ? "border-red-200 bg-red-50/30"
                    : wasmFile.status === "success"
                    ? "border-emerald-200 bg-emerald-50/30"
                    : "border-slate-200"
                )}
              >
                <div className="flex items-start gap-3">
                  {/* Icon */}
                  <div
                    className={cn(
                      "w-10 h-10 rounded-lg flex items-center justify-center shrink-0",
                      wasmFile.status === "success"
                        ? "bg-emerald-100 text-emerald-600"
                        : wasmFile.status === "error"
                        ? "bg-red-100 text-red-600"
                        : wasmFile.status === "uploading" || wasmFile.status === "validating"
                        ? "bg-indigo-100 text-indigo-600"
                        : "bg-slate-100 text-slate-500"
                    )}
                  >
                    {wasmFile.status === "uploading" || wasmFile.status === "validating" ? (
                      <Loader2 className="w-5 h-5 animate-spin" />
                    ) : wasmFile.status === "success" ? (
                      <CheckCircle2 className="w-5 h-5" />
                    ) : wasmFile.status === "error" ? (
                      <AlertCircle className="w-5 h-5" />
                    ) : (
                      <FileUp className="w-5 h-5" />
                    )}
                  </div>

                  {/* Info */}
                  <div className="flex-1 min-w-0">
                    <div className="flex items-center gap-2">
                      <p className="text-sm font-medium text-slate-800 truncate">
                        {wasmFile.file.name}
                      </p>
                      <span className="text-xs text-slate-400 shrink-0">
                        {(wasmFile.file.size / 1024).toFixed(1)} KB
                      </span>
                    </div>

                    {/* progress bar */}
                    {(wasmFile.status === "uploading" ||
                      wasmFile.status === "validating") && (
                    {/* Progress Bar */}
                    {wasmFile.status === "uploading" && (
                      <div className="mt-2">
                        <div className="h-1.5 bg-slate-100 rounded-full overflow-hidden">
                          <motion.div
                            className="h-full bg-indigo-500 rounded-full"
                            initial={{ width: 0 }}
                            animate={{ width: `${wasmFile.progress}%` }}
                            transition={{ duration: 0.3 }}
                          />
                        </div>
                        <p className="text-xs text-slate-400 mt-1">
                          {wasmFile.status === "validating"
                            ? "Decoding WASM module..."
                            : `Uploading... ${wasmFile.progress}%`}
                        </p>
                      </div>
                    )}

                    {/* Success State */}
                    {wasmFile.status === "success" && wasmFile.hash && (
                      <div className="mt-1.5 flex items-center gap-1.5">
                        <code className="text-xs font-mono text-emerald-700 bg-emerald-100 px-2 py-0.5 rounded truncate max-w-[200px]">
                          {wasmFile.hash.slice(0, 16)}...
                        </code>
                        <span className="text-xs text-emerald-600">
                          WASM hash ready
                        </span>
                      </div>
                    )}

                    {/* decode summary from the worker */}
                    {wasmFile.status === "success" && wasmFile.validation && (
                      <p className="mt-1 text-xs text-slate-500">
                        {wasmFile.validation.sections.length} sections &bull;{" "}
                        {extractContractFunctions(wasmFile.validation).length} exported
                        functions &bull; decoded in {wasmFile.validation.durationMs}ms
                      </p>
                    )}

                    {/* error state */}
                    {/* Error State */}
                    {wasmFile.status === "error" && wasmFile.error && (
                      <div className="mt-1.5 flex items-center gap-2">
                        <span className="text-xs text-red-600">
                          {wasmFile.error}
                        </span>
                        <button
                          onClick={() => retryUpload(wasmFile.id)}
                          className="text-xs text-indigo-600 hover:text-indigo-700 font-medium"
                        >
                          Retry
                        </button>
                      </div>
                    )}
                  </div>

                  {/* Actions */}
                  <div className="flex items-center gap-1">
                    {wasmFile.status === "success" && (
                      <button
                        onClick={() => {
                          console.log("Analyze WASM:", wasmFile.hash);
                        }}
                        className="p-1.5 text-emerald-600 hover:bg-emerald-50 rounded-lg transition-colors"
                        title="Analyze contract"
                      >
                        <ChevronRight className="w-4 h-4" />
                      </button>
                    )}
                    <button
                      onClick={() => removeFile(wasmFile.id)}
                      className="p-1.5 text-slate-400 hover:text-red-500 hover:bg-red-50 rounded-lg transition-colors"
                      title="Remove file"
                    >
                      <X className="w-4 h-4" />
                    </button>
                  </div>
                </div>
              </motion.div>
            ))}

            {/* Analyze All Button */}
            {successCount > 0 && (
              <motion.button
                initial={{ opacity: 0 }}
                animate={{ opacity: 1 }}
                onClick={() => {
                  const completed = files.filter(
                    (f) => f.status === "success"
                  );
                  onUploadComplete?.(completed);
                }}
                className="w-full mt-4 bg-indigo-600 hover:bg-indigo-700 text-white font-medium py-3 px-4 rounded-xl transition-colors flex items-center justify-center gap-2"
              >
                <FileCode className="w-5 h-5" />
                Analyze {successCount} Contract{successCount !== 1 ? "s" : ""}
              </motion.button>
            )}
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}