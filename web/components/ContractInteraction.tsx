import React, { useCallback, useEffect, useRef } from 'react';
import { ContractFunction, SimulationInputs } from '../lib/sorobantypes';
import { DynamicForm } from './DynamicForm';

interface ContractInteractionProps {
    selectedFunction: ContractFunction;
    loading: boolean;
    onSubmit: (inputs: SimulationInputs, signal?: AbortSignal) => Promise<void>;
}

export const ContractInteraction: React.FC<ContractInteractionProps> = ({
    selectedFunction,
    loading,
    onSubmit,
}) => {
    const requestIdRef = useRef<number>(0);
    const abortControllerRef = useRef<AbortController | null>(null);

    useEffect(() => {
        return () => {
            if (abortControllerRef.current) {
                abortControllerRef.current.abort();
                abortControllerRef.current = null;
            }
        };
    }, []);

    const handleSubmit = useCallback(
        async (inputs: SimulationInputs): Promise<void> => {
            const requestId = ++requestIdRef.current;

            if (abortControllerRef.current) {
                abortControllerRef.current.abort();
            }

            const controller = new AbortController();
            abortControllerRef.current = controller;

            try {
                await onSubmit(inputs, controller.signal);
            } catch (err) {
                if (
                    controller.signal.aborted ||
                    (err instanceof Error && err.name === 'AbortError')
                ) {
                    return;
                }
                throw err;
            } finally {
                if (requestId !== requestIdRef.current) {
                    // A newer request has superseded this one; ignore results.
                    return;
                }
                if (abortControllerRef.current === controller) {
                    abortControllerRef.current = null;
                }
            }
        },
        [onSubmit]
    );

    return (
        <div
            style={{
                backgroundColor: 'var(--bg-card)',
                borderRadius: '8px',
                padding: '24px',
                border: '1px solid var(--border-default)',
            }}
        >
            <h2
                style={{
                    margin: '0 0 16px 0',
                    fontSize: '18px',
                    fontWeight: '600',
                    color: '#58a6ff',
                }}
            >
                {selectedFunction.name}
            </h2>
            <DynamicForm
                func={selectedFunction}
                onSubmit={handleSubmit}
                loading={loading}
            />
        </div>
    );
};