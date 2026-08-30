import React from 'react';
import { ContractFunction } from '../lib/sorobantypes';

interface FunctionSidebarProps {
    functions: ContractFunction[];
    selectedFunction: ContractFunction;
    onSelect: (func: ContractFunction) => void;
}

export const FunctionSidebar: React.FC<FunctionSidebarProps> = ({
    functions,
    selectedFunction,
    onSelect,
}) => {
    return (
        <div
            style={{
                backgroundColor: '#161b22',
                borderRadius: '8px',
                padding: '24px',
                marginBottom: '24px',
                border: '1px solid #30363d',
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
                Available Functions
            </h2>
            <div style={{ display: 'flex', flexDirection: 'column', gap: '8px' }}>
                {functions.map((func) => {
                    const isSelected = selectedFunction.name === func.name;
                    return (
                        <button
                            key={func.name}
                            onClick={() => onSelect(func)}
                            style={{
                                padding: '12px 16px',
                                backgroundColor: isSelected ? '#8957e5' : '#21262d',
                                color: isSelected ? '#fff' : '#c9d1d9',
                                border: isSelected ? '1px solid #8957e5' : '1px solid #30363d',
                                borderRadius: '6px',
                                textAlign: 'left',
                                cursor: 'pointer',
                                fontWeight: isSelected ? '600' : '500',
                                transition: 'all 0.2s',
                                fontSize: '14px',
                                display: 'flex',
                                justifyContent: 'space-between',
                                alignItems: 'center',
                            }}
                            onMouseEnter={(e) => {
                                if (!isSelected) {
                                    e.currentTarget.style.backgroundColor = '#21262d';
                                    e.currentTarget.style.borderColor = '#8957e5';
                                }
                            }}
                            onMouseLeave={(e) => {
                                if (!isSelected) {
                                    e.currentTarget.style.backgroundColor = '#21262d';
                                    e.currentTarget.style.borderColor = '#30363d';
                                }
                            }}
                        >
                            <span>{func.name}</span>
                            <span style={{ fontSize: '12px', opacity: '0.7' }}>
                                {func.inputs.length} args
                            </span>
                        </button>
                    );
                })}
            </div>
        </div>
    );
};
