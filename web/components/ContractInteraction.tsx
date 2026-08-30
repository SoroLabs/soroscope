import React from 'react';
import { ContractFunction, SimulationInputs } from '../lib/sorobantypes';
import { DynamicForm } from './DynamicForm';

interface ContractInteractionProps {
    selectedFunction: ContractFunction;
    loading: boolean;
    onSubmit: (inputs: SimulationInputs) => Promise<void>;
}

export const ContractInteraction: React.FC<ContractInteractionProps> = ({
    selectedFunction,
    loading,
    onSubmit,
}) => {
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
                onSubmit={onSubmit}
                loading={loading}
            />
        </div>
    );
};
