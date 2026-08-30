import React from 'react';
import { Activity, Cpu, Database, HardDrive } from 'lucide-react';

interface ResourceMetric {
    label: string;
    value: number;
    max: number;
    unit: string;
    color: string;
    icon: React.ElementType;
}

interface NutritionLabelProps {
    cpu_instructions: number;
    ram_bytes: number;
    ledger_read_bytes: number;
    ledger_write_bytes: number;
    transaction_size_bytes: number;
}

export const NutritionLabel: React.FC<NutritionLabelProps> = ({
    cpu_instructions,
    ram_bytes,
    ledger_read_bytes,
    ledger_write_bytes,
    transaction_size_bytes,
}) => {
    // Soroban Limits (Approximate / Configurable)
    const LIMITS = {
        CPU: 100_000_000, // 100M instructions
        RAM: 40 * 1024 * 1024, // 40MB
        LEDGER_READ: 64 * 1024, // 64KB
        LEDGER_WRITE: 64 * 1024, // 64KB
        TX_SIZE: 128 * 1024, // 128KB
    };

    const metrics: ResourceMetric[] = [
        {
            label: 'CPU Instructions',
            value: cpu_instructions,
            max: LIMITS.CPU,
            unit: 'instr',
            color: '#ef4444', // Red-500
            icon: Cpu,
        },
        {
            label: 'Memory (RAM)',
            value: ram_bytes,
            max: LIMITS.RAM,
            unit: 'bytes',
            color: '#eab308', // Yellow-500
            icon: Activity,
        },
        {
            label: 'Ledger Reads',
            value: ledger_read_bytes,
            max: LIMITS.LEDGER_READ,
            unit: 'bytes',
            color: '#3b82f6', // Blue-500
            icon: Database,
        },
        {
            label: 'Ledger Writes',
            value: ledger_write_bytes,
            max: LIMITS.LEDGER_WRITE,
            unit: 'bytes',
            color: '#10b981', // Emerald-500
            icon: HardDrive,
        },
        {
            label: 'Transaction Size',
            value: transaction_size_bytes,
            max: LIMITS.TX_SIZE,
            unit: 'bytes',
            color: '#a371f7', // Purple-500
            icon: Activity, // Reuse icon or find another
        },
    ];

    const formatNumber = (num: number) => {
        return new Intl.NumberFormat('en-US', { notation: "compact", compactDisplay: "short" }).format(num);
    };

    return (
        <div className="bg-[#161b22] border border-[#30363d] rounded-lg p-4 sm:p-6 font-mono">
            <div className="border-b-2 border-[#30363d] pb-2 mb-4 flex flex-wrap justify-between items-end gap-2">
                <h2 className="text-xl sm:text-2xl font-black text-[#c9d1d9] uppercase tracking-wider">Nutrition Facts</h2>
                <span className="text-xs text-[#8b949e]">Per Transaction</span>
            </div>

            <div className="space-y-4">
                {metrics.map((metric) => {
                    const percentage = Math.min((metric.value / metric.max) * 100, 100);

                    return (
                        <div key={metric.label} className="group">
                            <div className="flex justify-between items-center mb-1">
                                <div className="flex items-center gap-2 text-[#c9d1d9]">
                                    <metric.icon size={16} className="text-[#8b949e]" />
                                    <span className="font-bold text-sm">{metric.label}</span>
                                </div>
                                <div className="text-right">
                                    <span className="font-bold text-[#c9d1d9]">{formatNumber(metric.value)}</span>
                                    <span className="text-xs text-[#8b949e] ml-1">{metric.unit}</span>
                                </div>
                            </div>

                            {/* Progress Bar Container */}
                            <div className="h-2 w-full bg-[#0d1117] rounded-full overflow-hidden border border-[#30363d]">
                                <div
                                    className="h-full rounded-full transition-all duration-500 ease-out"
                                    style={{
                                        width: `${percentage}%`,
                                        backgroundColor: metric.color
                                    }}
                                />
                            </div>

                            <div className="flex justify-end mt-1">
                                <span className="text-[10px] text-[#8b949e]">{percentage.toFixed(1)}% of Limit</span>
                            </div>
                            <div className="h-[1px] bg-[#30363d] mt-2 group-last:hidden" />
                        </div>
                    );
                })}
            </div>

            <div className="mt-6 pt-4 border-t-[4px] border-[#30363d]">
                <p className="text-[10px] text-[#8b949e] leading-tight">
                    * Percent Daily Values are based on current Soroban network protocol limits. Your limits may vary based on protocol version.
                </p>
            </div>
        </div>
    );
};
