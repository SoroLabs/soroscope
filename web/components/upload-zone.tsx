"use client"; 

import React, { useCallback, useState } from 'react';
import { useDropzone, FileRejection } from 'react-dropzone';
import { Upload, FileCode, XCircle, CheckCircle, Loader2 } from 'lucide-react';
import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';

function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}


type UploadStatus = 'idle' | 'scanning' | 'success' | 'error';

export function WasmUploadZone() {

  const [file, setFile] = useState<File | null>(null);
  const [status, setStatus] = useState<UploadStatus>('idle');
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  const onDrop = useCallback((acceptedFiles: File[], fileRejections: FileRejection[]) => {

    setErrorMessage(null);
    setStatus('idle');

    if (fileRejections.length > 0) {
      setStatus('error');
      
      const firstRejection = fileRejections[0];
      const rejectionErrors = firstRejection.errors;
      setErrorMessage(rejectionErrors.map(e => e.message).join(', '));

      return;
    }

    if (acceptedFiles.length > 0) {
      const uploadedFile = acceptedFiles[0];
      setFile(uploadedFile);
      setStatus('scanning');

      // 3. Simulate Scanning (Mocking the API call)
      setTimeout(() => {
        setStatus('success');
      }, 2000);
    }
  }, []);

  const { getRootProps, getInputProps, isDragActive, isDragReject } = useDropzone({
    onDrop,
    accept: { 'application/wasm': ['.wasm'] },
    maxFiles: 1,
    multiple: false,
  });

  const formatFileSize = (bytes: number): string => {
    if (bytes === 0) return '0 Bytes';
    const k = 1024;
    const sizes = ['Bytes', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
  };

  return (
    <div className="w-full max-w-2xl mx-auto mt-10">
      <div
        {...getRootProps()}
        className={cn(
          "relative flex flex-col items-center justify-center w-full h-72 border-2 border-dashed rounded-xl cursor-pointer transition-all duration-300 ease-in-out bg-slate-50",
          "border-slate-300 hover:bg-slate-100",

          isDragActive && !isDragReject && "border-indigo-500 bg-indigo-50 ring-2 ring-indigo-200 ring-offset-2",
          isDragReject && "border-red-500 bg-red-50",
          status === 'success' && "border-green-500 bg-green-50/50"
        )}
      >
        <input {...getInputProps()} />

        <div className="flex flex-col items-center justify-center pt-5 pb-6 text-center space-y-3">
          
          {status === 'scanning' ? (
            <>
              <Loader2 className="w-12 h-12 text-indigo-600 animate-spin" />
              <div>
                <p className="text-lg font-semibold text-indigo-900">Scanning Contract...</p>
                <p className="text-sm text-slate-500">Parsing resource metrics</p>
              </div>
            </>
          ) : 
          
          status === 'success' && file ? (
            <>
              <div className="p-3 bg-green-100 rounded-full">
                <CheckCircle className="w-8 h-8 text-green-600" />
              </div>
              <div>
                <p className="text-lg font-semibold text-green-900">Ready for Analysis</p>
                <div className="flex items-center justify-center gap-2 mt-2 px-3 py-1 bg-white border border-green-200 rounded-md shadow-sm">
                  <FileCode className="w-4 h-4 text-slate-500" />
                  <span className="text-sm font-medium text-slate-700">{file.name}</span>
                  <span className="text-xs text-slate-400">({formatFileSize(file.size)})</span>
                </div>
              </div>
            </>
          ) : 
          
          status === 'error' || isDragReject ? (
            <>
              <div className="p-3 bg-red-100 rounded-full">
                <XCircle className="w-8 h-8 text-red-500" />
              </div>
              <div>
                <p className="text-lg font-semibold text-red-700">Upload Failed</p>
                <p className="text-sm text-red-500 mt-1">{errorMessage || "File type not accepted"}</p>
              </div>
            </>
          ) : 
          
          (
            <>
              <div className={cn(
                "p-4 rounded-full bg-indigo-50 transition-colors",
                isDragActive ? "bg-indigo-100" : ""
              )}>
                <Upload className={cn("w-10 h-10 text-indigo-500", isDragActive && "scale-110 duration-200")} />
              </div>
              <div className="space-y-1">
                <p className="text-base font-semibold text-slate-700">
                  <span className="text-indigo-600 hover:underline">Click to upload</span> or drag and drop
                </p>
                <p className="text-sm text-slate-500">
                  Soroban WebAssembly (.wasm) only
                </p>
              </div>
            </>
          )}
        </div>
      </div>
    </div>
  );
}