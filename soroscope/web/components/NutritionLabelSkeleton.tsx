import React from 'react';
import { Cpu, Activity, Database, HardDrive } from 'lucide-react';

export const NutritionLabelSkeleton: React.FC = () => {
    const dummyRows = [
        { label: 'CPU Instructions', icon: Cpu },
        { label: 'Memory (RAM)', icon: Activity },
        { label: 'Ledger Reads', icon: Database },
        { label: 'Ledger Writes', icon: HardDrive },
        { label: 'Transaction Size', icon: Activity },
    ];

    return (
        <div className="bg-[#161b22] border border-[#30363d] rounded-lg p-6 font-mono animate-pulse">
            <div className="border-b-2 border-[#30363d] pb-2 mb-4 flex justify-between items-end">
                <h2 className="text-2xl font-black text-[#c9d1d9] uppercase tracking-wider">Nutrition Facts</h2>
                <span className="text-xs text-[#8b949e]">Per Transaction</span>
            </div>

            <div className="space-y-4">
                {dummyRows.map((row, idx) => (
                    <div key={idx} className="group">
                        <div className="flex justify-between items-center mb-1">
                            <div className="flex items-center gap-2">
                                <row.icon size={16} className="text-[#30363d]" />
                                <div className="h-4 w-32 bg-[#30363d] rounded" />
                            </div>
                            <div className="flex items-center gap-1">
                                <div className="h-4 w-12 bg-[#30363d] rounded" />
                            </div>
                        </div>

                        {/* Progress Bar Container */}
                        <div className="h-2 w-full bg-[#0d1117] rounded-full overflow-hidden border border-[#30363d]">
                            <div className="h-full w-1/3 bg-[#30363d] rounded-full" />
                        </div>

                        <div className="flex justify-end mt-1">
                            <div className="h-3 w-16 bg-[#30363d] rounded mt-0.5" />
                        </div>
                        {idx < dummyRows.length - 1 && <div className="h-[1px] bg-[#30363d] mt-2" />}
                    </div>
                ))}
            </div>

            <div className="mt-6 pt-4 border-t-[4px] border-[#30363d] flex flex-col gap-1.5">
                <div className="h-2.5 w-full bg-[#30363d] rounded" />
                <div className="h-2.5 w-5/6 bg-[#30363d] rounded" />
            </div>
        </div>
    );
};
