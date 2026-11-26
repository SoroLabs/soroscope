import React, { useState, useRef, useEffect } from 'react';
import { ContractFunction } from '../lib/sorobantypes';
import { Search, ChevronDown, ChevronUp, ChevronLeft, ChevronRight, Code2, X } from 'lucide-react';

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
    const [isOpen, setIsOpen] = useState(true);
    const [searchQuery, setSearchQuery] = useState('');
    const [collapsed, setCollapsed] = useState(false);
    const searchRef = useRef<HTMLInputElement>(null);

    // Focus the search input when the sidebar opens
    useEffect(() => {
        if (isOpen && !collapsed && searchRef.current) {
            searchRef.current.focus();
        }
    }, [isOpen, collapsed]);

    const filteredFunctions = functions.filter((func) => {
        const query = searchQuery.toLowerCase().trim();
        if (!query) return true;
        const matchesName = func.name.toLowerCase().includes(query);
        const matchesInputs = func.inputs.some(
            (input) =>
                input.name.toLowerCase().includes(query) ||
                input.type.toLowerCase().includes(query) ||
                (input.description && input.description.toLowerCase().includes(query))
        );
        const matchesOutput = func.outputs && func.outputs.toLowerCase().includes(query);
        return matchesName || matchesInputs || matchesOutput;
    });

    const clearSearch = () => {
        setSearchQuery('');
        searchRef.current?.focus();
    };

    return (
        <div style={{ position: 'relative', marginBottom: '24px' }}>
            {/* Toggle collapse / expand sidebar button */}
            <button
                onClick={() => setIsOpen((prev) => !prev)}
                title={isOpen ? 'Hide functions sidebar' : 'Show functions sidebar'}
                style={{
                    position: 'absolute',
                    top: '16px',
                    right: '-14px',
                    zIndex: 10,
                    width: '28px',
                    height: '28px',
                    borderRadius: '50%',
                    backgroundColor: 'var(--bg-card)',
                    border: '1px solid #30363d',
                    cursor: 'pointer',
                    display: 'flex',
                    alignItems: 'center',
                    justifyContent: 'center',
                    color: 'var(--text-secondary)',
                    flexShrink: 0,
                }}
            >
                {isOpen ? <ChevronLeft size={14} /> : <ChevronRight size={14} />}
            </button>

            {isOpen && (
                <div
                    style={{
                        backgroundColor: 'var(--bg-card)',
                        borderRadius: '8px',
                        padding: '20px',
                        border: '1px solid #30363d',
                        display: 'flex',
                        flexDirection: 'column',
                        boxSizing: 'border-box',
                    }}
                >
                    <div
                        style={{
                            display: 'flex',
                            alignItems: 'center',
                            justifyContent: 'space-between',
                            marginBottom: '12px',
                        }}
                    >
                        <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                            <Code2 size={16} color="#58a6ff" />
                            <h2
                                style={{
                                    margin: 0,
                                    fontSize: '16px',
                                    fontWeight: '600',
                                    color: '#58a6ff',
                                }}
                            >
                                Exported Functions
                            </h2>
                            <span
                                style={{
                                    fontSize: '11px',
                                    backgroundColor: '#21262d',
                                    color: 'var(--text-secondary)',
                                    borderRadius: '10px',
                                    padding: '1px 6px',
                                    lineHeight: '16px',
                                }}
                            >
                                {searchQuery ? `${filteredFunctions.length}/${functions.length}` : functions.length}
                            </span>
                        </div>
                        <button
                            onClick={() => setCollapsed((prev) => !prev)}
                            style={{
                                background: 'none',
                                border: 'none',
                                color: 'var(--text-secondary)',
                                cursor: 'pointer',
                                display: 'flex',
                                alignItems: 'center',
                                padding: '2px',
                            }}
                            title={collapsed ? 'Expand list' : 'Collapse list'}
                        >
                            {collapsed ? <ChevronDown size={16} /> : <ChevronUp size={16} />}
                        </button>
                    </div>

                    {/* Search input — always visible when sidebar is open so users
                        can filter even before expanding the full list (fixes #1292). */}
                    <div style={{ position: 'relative', marginBottom: collapsed ? '0' : '14px' }}>
                        <Search
                            size={14}
                            color="#8b949e"
                            style={{
                                position: 'absolute',
                                left: '12px',
                                top: '50%',
                                transform: 'translateY(-50%)',
                                pointerEvents: 'none',
                            }}
                        />
                        <input
                            ref={searchRef}
                            type="search"
                            placeholder="Search functions, args or types…"
                            value={searchQuery}
                            onChange={(e) => {
                                setSearchQuery(e.target.value);
                                // Auto-expand list when user starts typing
                                if (e.target.value && collapsed) {
                                    setCollapsed(false);
                                }
                            }}
                            aria-label="Search exported functions"
                            style={{
                                width: '100%',
                                padding: '9px 36px 9px 34px',
                                borderRadius: '6px',
                                border: '1px solid #30363d',
                                backgroundColor: '#0d1117',
                                color: '#c9d1d9',
                                fontSize: '13px',
                                outline: 'none',
                                boxSizing: 'border-box',
                            }}
                        />
                        {searchQuery && (
                            <button
                                onClick={clearSearch}
                                aria-label="Clear search"
                                style={{
                                    position: 'absolute',
                                    right: '10px',
                                    top: '50%',
                                    transform: 'translateY(-50%)',
                                    background: 'none',
                                    border: 'none',
                                    cursor: 'pointer',
                                    color: '#8b949e',
                                    display: 'flex',
                                    alignItems: 'center',
                                    padding: '2px',
                                }}
                            >
                                <X size={13} />
                            </button>
                        )}
                    </div>

                    {!collapsed && (
                        <>
                            {/* Function list */}
                            <div
                                role="listbox"
                                aria-label="Contract functions"
                                style={{
                                    display: 'flex',
                                    flexDirection: 'column',
                                    gap: '8px',
                                    maxHeight: '320px',
                                    overflowY: 'auto',
                                }}
                            >
                                {filteredFunctions.length === 0 ? (
                                    <div
                                        style={{
                                            padding: '24px 16px',
                                            textAlign: 'center',
                                            color: 'var(--text-secondary)',
                                            fontSize: '13px',
                                        }}
                                    >
                                        No functions match &ldquo;{searchQuery}&rdquo;
                                    </div>
                                ) : (
                                    filteredFunctions.map((func) => {
                                        const isSelected = selectedFunction.name === func.name;
                                        return (
                                            <button
                                                key={func.name}
                                                role="option"
                                                aria-selected={isSelected}
                                                onClick={() => onSelect(func)}
                                                style={{
                                                    padding: '10px 14px',
                                                    backgroundColor: isSelected ? '#8957e5' : '#21262d',
                                                    color: isSelected ? '#fff' : '#c9d1d9',
                                                    border: isSelected ? '1px solid #8957e5' : '1px solid #30363d',
                                                    borderRadius: '6px',
                                                    textAlign: 'left',
                                                    cursor: 'pointer',
                                                    fontWeight: isSelected ? '600' : '500',
                                                    transition: 'all 0.2s',
                                                    fontSize: '13px',
                                                    display: 'flex',
                                                    justifyContent: 'space-between',
                                                    alignItems: 'center',
                                                }}
                                                onMouseEnter={(e) => {
                                                    if (!isSelected) {
                                                        e.currentTarget.style.backgroundColor = '#1c2128';
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
                                                <span
                                                    style={{
                                                        overflow: 'hidden',
                                                        textOverflow: 'ellipsis',
                                                        whiteSpace: 'nowrap',
                                                        marginRight: '8px',
                                                    }}
                                                >
                                                    {func.name}
                                                </span>
                                                <span
                                                    style={{
                                                        fontSize: '11px',
                                                        opacity: '0.8',
                                                        flexShrink: 0,
                                                        fontFamily: 'monospace',
                                                    }}
                                                >
                                                    {func.inputs.length} arg{func.inputs.length === 1 ? '' : 's'}
                                                </span>
                                            </button>
                                        );
                                    })
                                )}
                            </div>
                        </>
                    )}
                </div>
            )}
        </div>
    );
};
