'use client';

import React, { useEffect, useMemo, useState, useCallback } from 'react';
import clsx from 'clsx';
import { ExternalLink, Loader2, Download, Search, ArrowUp, ArrowDown, ArrowUpDown } from 'lucide-react';
import type { TransactionRecord, TransactionStatus } from '../lib/sorobantypes';
import { paginate } from '../lib/paginationUtils';
import { DEFAULT_TRANSACTION_FILTER, filterTransactions, type TransactionFilter } from '../lib/transactionFilters';
import { CopyButton } from './CopyButton';
import { useInfiniteScroll } from '../hooks/useInfiniteScroll';

const PER_PAGE = 10;
const PAGE_SIZE_OPTIONS = [10, 25, 50] as const;

type SortKey = 'timestamp' | 'fee' | 'status';
type SortDirection = 'asc' | 'desc';

const STATUS_ORDER: Record<TransactionStatus, number> = {
  success: 0,
  pending: 1,
  failed: 2,
};

function statusBadge(status: TransactionStatus) {
  const style =
    status === 'success'
      ? 'border-emerald-500/50 bg-emerald-500/10 text-emerald-200'
      : status === 'failed'
        ? 'border-red-500/50 bg-red-500/10 text-red-200'
        : 'border-yellow-500/50 bg-yellow-500/10 text-yellow-200';

  const label = status === 'success' ? 'Success' : status === 'failed' ? 'Failed' : 'Pending';

  return (
    <span
      className={clsx(
        'inline-flex items-center rounded-full border px-2 py-0.5 text-[11px] font-semibold',
        style,
      )}
    >
      {label}
    </span>
  );
}

function SortableHeader({
  label,
  sortKey,
  activeKey,
  direction,
  onSort,
  className,
}: {
  label: string;
  sortKey: SortKey;
  activeKey: SortKey | null;
  direction: SortDirection;
  onSort: (key: SortKey) => void;
  className?: string;
}) {
  const isActive = activeKey === sortKey;
  const Icon = !isActive ? ArrowUpDown : direction === 'asc' ? ArrowUp : ArrowDown;

  return (
    <th scope="col" className={clsx('px-4 py-3 text-left', className)}>
      <button
        type="button"
        onClick={() => onSort(sortKey)}
        aria-label={`Sort by ${label}`}
        aria-sort={isActive ? (direction === 'asc' ? 'ascending' : 'descending') : 'none'}
        className={clsx(
          'inline-flex items-center gap-1 text-xs font-semibold uppercase tracking-wide transition-colors',
          isActive ? 'text-cyan-400' : 'text-[#8b949e] hover:text-[#c9d1d9]',
        )}
      >
        {label}
        <Icon className="h-3 w-3" />
      </button>
    </th>
  );
}

function PaginationButton({
  children,
  active,
  disabled,
  onClick,
}: {
  children: React.ReactNode;
  active?: boolean;
  disabled?: boolean;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={disabled}
      className={clsx(
        'min-w-[32px] rounded border px-2.5 py-1.5 text-xs font-medium transition-colors',
        active
          ? 'border-cyan-500/50 bg-cyan-500/10 text-cyan-400'
          : 'border-[#30363d] bg-[#161b22] text-[#8b949e] hover:border-[#8b949e] hover:text-[#c9d1d9]',
        disabled && 'cursor-not-allowed opacity-40',
      )}
    >
      {children}
    </button>
  );
}

function SkeletonRow() {
  return (
    <tr className="animate-pulse">
      <td className="px-4 py-3">
        <div className="h-4 w-64 rounded bg-slate-800" />
      </td>
      <td className="px-4 py-3">
        <div className="h-4 w-20 rounded bg-slate-800" />
      </td>
      <td className="px-4 py-3">
        <div className="h-5 w-16 rounded-full bg-slate-800" />
      </td>
      <td className="px-4 py-3">
        <div className="h-4 w-24 rounded bg-slate-800" />
      </td>
      <td className="px-4 py-3">
        <div className="h-4 w-16 rounded bg-slate-800" />
      </td>
      <td className="px-4 py-3">
        <div className="h-4 w-12 rounded bg-slate-800" />
      </td>
    </tr>
  );
}

interface TransactionHistoryTableProps {
  transactions: TransactionRecord[];
  loading?: boolean;
  /** Whether to use Infinite Scroll pagination for event logs (default: true). */
  enableInfiniteScroll?: boolean;
  /** Active status/function-name filter. Must be memoized by the caller (e.g. via useMemo). */
  filter?: TransactionFilter;
  /** Called when the status filter changes. Should be memoized by the caller (e.g. via useCallback). */
  onStatusFilterChange?: (status: TransactionStatus | 'all') => void;
  /** Called when the function-name filter changes. Should be memoized by the caller (e.g. via useCallback). */
  onFunctionFilterChange?: (functionName: string) => void;
}

export function TransactionHistoryTable({
  transactions,
  loading = false,
  enableInfiniteScroll = true,
  filter = DEFAULT_TRANSACTION_FILTER,
  onStatusFilterChange,
  onFunctionFilterChange,
}: TransactionHistoryTableProps) {
  const [page, setPage] = useState(1);
  const [pageSize, setPageSize] = useState<number>(PER_PAGE);
  const [visibleLimit, setVisibleLimit] = useState(PER_PAGE);
  const [isFetchingMore, setIsFetchingMore] = useState(false);
  const [isInfiniteMode, setIsInfiniteMode] = useState(enableInfiniteScroll);
  const [searchQuery, setSearchQuery] = useState('');
  const [sortKey, setSortKey] = useState<SortKey | null>(null);
  const [sortDirection, setSortDirection] = useState<SortDirection>('desc');

  const filteredTransactions = useMemo(
    () => filterTransactions(transactions, filter),
    [transactions, filter],
  );

  // Real-time text search across contract address and method name.
  const searchedTransactions = useMemo(() => {
    const query = searchQuery.trim().toLowerCase();
    if (!query) return filteredTransactions;
    return filteredTransactions.filter((tx) => {
      const address = (tx.contractAddress ?? '').toLowerCase();
      const method = (tx.functionName ?? '').toLowerCase();
      return address.includes(query) || method.includes(query);
    });
  }, [filteredTransactions, searchQuery]);

  const sortedTransactions = useMemo(() => {
    if (!sortKey) return searchedTransactions;
    const dir = sortDirection === 'asc' ? 1 : -1;
    return [...searchedTransactions].sort((a, b) => {
      if (sortKey === 'timestamp') {
        return (new Date(a.timestamp).getTime() - new Date(b.timestamp).getTime()) * dir;
      }
      if (sortKey === 'fee') {
        return ((a.fee ?? 0) - (b.fee ?? 0)) * dir;
      }
      return (STATUS_ORDER[a.status] - STATUS_ORDER[b.status]) * dir;
    });
  }, [searchedTransactions, sortKey, sortDirection]);

  const handleSort = useCallback((key: SortKey) => {
    setSortKey((prevKey) => {
      if (prevKey === key) {
        setSortDirection((prevDir) => (prevDir === 'asc' ? 'desc' : 'asc'));
        return prevKey;
      }
      setSortDirection('desc');
      return key;
    });
    setPage(1);
    setVisibleLimit(PER_PAGE);
  }, []);

  // Jump back to the first page/batch whenever the filter itself changes.
  // Relies on `filter` being a stable (memoized) reference from the parent —
  // otherwise a new object on every render would re-trigger this on every render too.
  useEffect(() => {
    setPage(1);
    setVisibleLimit(PER_PAGE);
  }, [filter, searchQuery, pageSize]);

  const { items: pageItems, page: currentPage, totalPages, total } = useMemo(
    () => paginate(sortedTransactions, page, pageSize),
    [sortedTransactions, page, pageSize],
  );

  const hasMore = visibleLimit < sortedTransactions.length;

  const handleLoadMore = useCallback(() => {
    if (isFetchingMore || !hasMore) return;
    setIsFetchingMore(true);
    setTimeout(() => {
      setVisibleLimit((prev) => Math.min(sortedTransactions.length, prev + PER_PAGE));
      setIsFetchingMore(false);
    }, 300);
  }, [isFetchingMore, hasMore, sortedTransactions.length]);

  const sentinelRef = useInfiniteScroll({
    onLoadMore: handleLoadMore,
    hasMore: isInfiniteMode && hasMore,
    isLoading: isFetchingMore,
  });

  const visibleTransactions = useMemo(() => {
    return isInfiniteMode ? sortedTransactions.slice(0, visibleLimit) : pageItems;
  }, [isInfiniteMode, sortedTransactions, visibleLimit, pageItems]);

  const explorerUrl =
    process.env.NEXT_PUBLIC_STELLAR_EXPLORER_URL ?? 'https://stellar.expert/explorer/testnet';

  const exportToCSV = useCallback(() => {
    if (!transactions.length) return;
    const headers = ['Transaction Hash', 'Function', 'Status', 'Timestamp', 'Fee (XLM)'];
    const escape = (val: string) => {
      const clean = val.replace(/"/g, '""');
      return `"${clean}"`;
    };
    const rows = transactions.map((tx) => [
      escape(tx.hash),
      escape(tx.functionName),
      escape(tx.status),
      escape(new Date(tx.timestamp).toISOString()),
      escape(tx.fee ? `${tx.fee}` : '0'),
    ]);
    const csvContent = [headers.join(','), ...rows.map((row) => row.join(','))].join('\n');
    const blob = new Blob([csvContent], { type: 'text/csv;charset=utf-8;' });
    const url = URL.createObjectURL(blob);
    const link = document.createElement('a');
    link.setAttribute('href', url);
    link.setAttribute('download', `telemetry_events_${Date.now()}.csv`);
    document.body.appendChild(link);
    link.click();
    document.body.removeChild(link);
    URL.revokeObjectURL(url);
  }, [transactions]);

  const exportToJSON = useCallback(() => {
    const jsonContent = JSON.stringify(transactions, null, 2);
    const blob = new Blob([jsonContent], { type: 'application/json;charset=utf-8;' });
    const url = URL.createObjectURL(blob);
    const link = document.createElement('a');
    link.setAttribute('href', url);
    link.setAttribute('download', `telemetry_events_${Date.now()}.json`);
    document.body.appendChild(link);
    link.click();
    document.body.removeChild(link);
    URL.revokeObjectURL(url);
  }, [transactions]);

  const hasActiveFilter = filter.status !== 'all' || filter.functionName.trim().length > 0;

  const filterControls = (onStatusFilterChange || onFunctionFilterChange) && (
    <div className="flex flex-col gap-2 border-b border-[#30363d] px-4 py-3 sm:flex-row sm:items-center">
      <div className="relative flex-1">
        <Search className="pointer-events-none absolute left-2.5 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-[#8b949e]" />
        <input
          type="text"
          value={filter.functionName}
          onChange={(e) => onFunctionFilterChange?.(e.target.value)}
          placeholder="Filter by function name..."
          aria-label="Filter by function name"
          className="w-full rounded border border-[#30363d] bg-[#161b22] py-1.5 pl-8 pr-2 text-xs text-[#c9d1d9] placeholder:text-[#6e7681] focus:outline-none focus:ring-1 focus:ring-cyan-500/50"
        />
      </div>
      <select
        value={filter.status}
        onChange={(e) => onStatusFilterChange?.(e.target.value as TransactionStatus | 'all')}
        aria-label="Filter by status"
        className="rounded border border-[#30363d] bg-[#161b22] px-2 py-1.5 text-xs text-[#c9d1d9] focus:outline-none focus:ring-1 focus:ring-cyan-500/50"
      >
        <option value="all">All statuses</option>
        <option value="success">Success</option>
        <option value="failed">Failed</option>
        <option value="pending">Pending</option>
      </select>
    </div>
  );

  const searchControls = (
    <div className="border-b border-[#30363d] px-4 py-3">
      <div className="relative">
        <Search className="pointer-events-none absolute left-2.5 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-[#8b949e]" />
        <input
          type="text"
          value={searchQuery}
          onChange={(e) => setSearchQuery(e.target.value)}
          placeholder="Search by contract address or method name..."
          aria-label="Search by contract address or method name"
          className="w-full rounded border border-[#30363d] bg-[#161b22] py-1.5 pl-8 pr-2 text-xs text-[#c9d1d9] placeholder:text-[#6e7681] focus:outline-none focus:ring-1 focus:ring-cyan-500/50"
        />
      </div>
    </div>
  );

  const paginationControls = !isInfiniteMode && totalPages > 1 && (
    <div className="flex flex-wrap items-center justify-between gap-3 border-t border-[#30363d] px-4 py-3">
      <div className="flex items-center gap-2 text-xs text-[#8b949e]">
        <span>Rows per page</span>
        <select
          value={pageSize}
          onChange={(e) => setPageSize(Number(e.target.value))}
          aria-label="Rows per page"
          className="rounded border border-[#30363d] bg-[#161b22] px-2 py-1 text-xs text-[#c9d1d9] focus:outline-none focus:ring-1 focus:ring-cyan-500/50"
        >
          {PAGE_SIZE_OPTIONS.map((size) => (
            <option key={size} value={size}>
              {size}
            </option>
          ))}
        </select>
      </div>
      <div className="flex items-center gap-1.5">
        <PaginationButton disabled={currentPage <= 1} onClick={() => setPage(currentPage - 1)}>
          Prev
        </PaginationButton>
        {Array.from({ length: totalPages }, (_, i) => i + 1).map((p) => (
          <PaginationButton key={p} active={p === currentPage} onClick={() => setPage(p)}>
            {p}
          </PaginationButton>
        ))}
        <PaginationButton
          disabled={currentPage >= totalPages}
          onClick={() => setPage(currentPage + 1)}
        >
          Next
        </PaginationButton>
      </div>
    </div>
  );

  if (!loading && transactions.length === 0) {
    return (
      <div className="rounded-lg border border-[#30363d] bg-[#0d1117] p-8 text-center text-sm text-[#8b949e]">
        No transactions yet.
      </div>
    );
  }

  return (
    <div className="overflow-hidden rounded-lg border border-[#30363d] bg-[#0d1117]">
      {filterControls}
      {searchControls}
      <div className="overflow-x-auto">
        <table className="w-full border-collapse text-sm">
          <thead className="border-b border-[#30363d] bg-[#161b22]">
            <tr>
              <th scope="col" className="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wide text-[#8b949e]">
                Transaction
              </th>
              <th scope="col" className="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wide text-[#8b949e]">
                Method
              </th>
              <SortableHeader
                label="Status"
                sortKey="status"
                activeKey={sortKey}
                direction={sortDirection}
                onSort={handleSort}
              />
              <SortableHeader
                label="Timestamp"
                sortKey="timestamp"
                activeKey={sortKey}
                direction={sortDirection}
                onSort={handleSort}
              />
              <SortableHeader
                label="Gas Cost"
                sortKey="fee"
                activeKey={sortKey}
                direction={sortDirection}
                onSort={handleSort}
              />
              <th scope="col" className="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wide text-[#8b949e]">
                Actions
              </th>
            </tr>
          </thead>
          <tbody>
            {loading
              ? Array.from({ length: 5 }).map((_, i) => <SkeletonRow key={i} />)
              : visibleTransactions.map((tx) => (
                  <tr key={tx.hash} className="border-b border-[#21262d] last:border-0 hover:bg-[#161b22]">
                    <td className="px-4 py-3 font-mono text-xs text-[#c9d1d9]">
                      <div className="flex items-center gap-2">
                        <span className="truncate">{tx.hash}</span>
                        <CopyButton text={tx.hash} />
                      </div>
                    </td>
                    <td className="px-4 py-3 text-xs text-[#c9d1d9]">{tx.functionName}</td>
                    <td className="px-4 py-3">{statusBadge(tx.status)}</td>
                    <td className="px-4 py-3 text-xs text-[#8b949e]">
                      {new Date(tx.timestamp).toLocaleString()}
                    </td>
                    <td className="px-4 py-3 text-xs text-[#8b949e]">
                      {tx.fee ? `${tx.fee} XLM` : '—'}
                    </td>
                    <td className="px-4 py-3">
                      <a
                        href={`${explorerUrl}/tx/${tx.hash}`}
                        target="_blank"
                        rel="noopener noreferrer"
                        className="inline-flex items-center gap-1 text-xs text-cyan-400 hover:text-cyan-300"
                      >
                        View <ExternalLink className="h-3 w-3" />
                      </a>
                    </td>
                  </tr>
                ))}
          </tbody>
        </table>
      </div>
      {!loading && visibleTransactions.length === 0 && (
        <div className="px-4 py-8 text-center text-sm text-[#8b949e]">
          No transactions match your search.
        </div>
      )}
      {paginationControls}
      {isInfiniteMode && hasMore && (
        <div ref={sentinelRef} className="flex items-center justify-center gap-2 py-4 text-xs text-[#8b949e]">
          {isFetchingMore ? (
            <>
              <Loader2 className="h-4 w-4 animate-spin" /> Loading more...
            </>
          ) : (
            'Scroll for more'
          )}
        </div>
      )}
      <div className="flex items-center justify-between border-t border-[#30363d] px-4 py-3">
        <span className="text-xs text-[#8b949e]">
          {total} transaction{total === 1 ? '' : 's'}
          {hasActiveFilter || searchQuery.trim() ? ' (filtered)' : ''}
        </span>
        <div className="flex items-center gap-2">
          <button
            type="button"
            onClick={() => setIsInfiniteMode((prev) => !prev)}
            className="rounded border border-[#30363d] bg-[#161b22] px-2.5 py-1.5 text-xs text-[#8b949e] hover:border-[#8b949e] hover:text-[#c9d1d9]"
          >
            {isInfiniteMode ? 'Paginate' : 'Infinite scroll'}
          </button>
          <button
            type="button"
            onClick={exportToCSV}
            className="inline-flex items-center gap-1 rounded border border-[#30363d] bg-[#161b22] px-2.5 py-1.5 text-xs text-[#8b949e] hover:border-[#8b949e] hover:text-[#c9d1d9]"
          >
            <Download className="h-3 w-3" /> CSV
          </button>
          <button
            type="button"
            onClick={exportToJSON}
            className="inline-flex items-center gap-1 rounded border border-[#30363d] bg-[#161b22] px-2.5 py-1.5 text-xs text-[#8b949e] hover:border-[#8b949e] hover:text-[#c9d1d9]"
          >
            <Download className="h-3 w-3" /> JSON
          </button>
        </div>
      </div>
    </div>
  );
}
