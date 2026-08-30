'use client';

import React, { useCallback, useState } from 'react';
import { motion } from 'framer-motion';
import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';

function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

interface WasmFile {
  file: File;
  id: string;
  status: 'pending' | 'uploading' | 'success' | 'error';
  progress: number;
  error?: string;
  hash?: string;
}

interface WasmUploadProps {
  onUploadComplete?: (files: WasmFile[]) => void;
  onFileSelect?: (files: File[]) => void;
  maxFileSize?: number;
  maxFiles?: number;
  className?: string;
}

export default function WasmUpload({
  onUploadComplete,
  onFileSelect,
  maxFileSize = 10 * 1024 * 1024,
  maxFiles = 5,
  className,
}: WasmUploadProps) {
  const [files, setFiles] = useState<WasmFile[]>([]);
  const [isDragActive, setIsDragActive] = useState(false);

  const validateWasm = useCallback((file: File): string | null => {
    if (!file.name.toLowerCase().endsWith('.wasm')) {
      return 'File must be a .wasm file';
    }
    if (file.size > maxFileSize) {
      return `File too large (max ${(maxFileSize / 1024 / 1024).toFixed(1)}MB)`;
    }
    if (file.size === 0) {
      return 'File is empty';
    }
    return null;
  }, [maxFileSize]);

  const onDrop = useCallback(
    (acceptedFiles: File[]) => {
      const newFiles: WasmFile[] = acceptedFiles.map((file) => ({
        file,
        id: `${file.name}-${Date.now()}-${Math.random().toString(36).slice(2)}`,
        status: 'pending',
        progress: 0,
      }));

      const validFiles: WasmFile[] = [];
      const invalidFiles: WasmFile[] = [];
      newFiles.forEach((wasmFile) => {
        const error = validateWasm(wasmFile.file);
        if (error) {
          invalidFiles.push({ ...wasmFile, status: 'error', error });
        } else {
          validFiles.push(wasmFile);
        }
      });

      const totalFiles = [...files, ...validFiles, ...invalidFiles];
      if (totalFiles.length > maxFiles) {
        window.alert(`Maximum ${maxFiles} files allowed`);
        return;
      }

      setFiles((prev) => [...prev, ...validFiles, ...invalidFiles]);
      onFileSelect?.(validFiles.map((file) => file.file));
      onUploadComplete?.([...validFiles, ...invalidFiles]);
      setIsDragActive(false);
    },
    [files, maxFiles, onFileSelect, onUploadComplete, validateWasm],
  );

  return (
    <div className={cn('w-full max-w-2xl mx-auto', className)}>
      <motion.div
        onDrop={(event) => {
          event.preventDefault();
          const droppedFiles = Array.from(event.dataTransfer.files);
          onDrop(droppedFiles);
        }}
        onDragOver={(event) => {
          event.preventDefault();
          setIsDragActive(true);
        }}
        onDragLeave={() => setIsDragActive(false)}
        className={cn(
          'relative rounded-2xl border-2 border-dashed p-8 text-center transition-colors duration-200',
          isDragActive ? 'border-violet-500 bg-violet-500/10' : 'border-slate-300 bg-slate-50/50 hover:border-slate-400',
        )}
      >
        <input
          type="file"
          accept=".wasm"
          multiple={maxFiles > 1}
          onChange={(event) => {
            const selected = Array.from(event.target.files ?? []);
            onDrop(selected);
          }}
        />
        <p className="text-sm text-slate-600">Drag and drop a .wasm contract here or click to select one.</p>
      </motion.div>
    </div>
  );
}
