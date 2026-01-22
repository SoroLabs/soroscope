/**
 * @jest-environment jsdom
 */


import React from 'react';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';
import { WasmUploadZone } from '../upload-zone';

describe('WasmUploadZone', () => {


  it('renders the initial idle state correctly', () => {
    render(<WasmUploadZone />);
    expect(screen.getByText(/Click to upload/i)).toBeInTheDocument();
    expect(screen.getByText(/Soroban WebAssembly \(\.wasm\) only/i)).toBeInTheDocument();
  });


  it('shows error state when a non-wasm file is dropped', async () => {
    const { container } = render(<WasmUploadZone />);
    
    const input = container.querySelector('input');
    if (!input) throw new Error('Hidden file input not found');

    const badFile = new File(['(⌐□_□)'], 'chucknorris.png', { type: 'image/png' });

    Object.defineProperty(input, 'files', { value: [badFile] });
    fireEvent.change(input);

    await waitFor(() => {
      expect(screen.getByText(/File type must be one of/i)).toBeInTheDocument();
    });
  });


  it('shows scanning state when a valid .wasm file is dropped', async () => {
    const { container } = render(<WasmUploadZone />);
    
    const input = container.querySelector('input');
    if (!input) throw new Error('Hidden file input not found');

    const goodFile = new File(['(wasm_magic_bytes)'], 'contract.wasm', { type: 'application/wasm' });

    Object.defineProperty(input, 'files', { value: [goodFile] });
    fireEvent.change(input);

    await waitFor(() => {
      expect(screen.getByText(/Scanning Contract.../i)).toBeInTheDocument();
    });
    

    await waitFor(() => {
      expect(screen.getByText(/Ready for Analysis/i)).toBeInTheDocument();
      expect(screen.getByText(/contract.wasm/i)).toBeInTheDocument();
    }, { timeout: 3000 });
  });

});