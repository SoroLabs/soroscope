export interface PaginationResult<T> {
  items: T[];
  page: number;
  perPage: number;
  total: number;
  totalPages: number;
}

export function paginate<T>(
  items: T[],
  page: number,
  perPage: number,
): PaginationResult<T> {
  const total = items.length;
  const totalPages = perPage > 0 ? Math.max(1, Math.ceil(total / perPage)) : 1;
  const clampedPage = Math.max(1, Math.min(page, totalPages));
  const start = (clampedPage - 1) * perPage;
  const end = start + perPage;

  return {
    items: items.slice(start, end),
    page: clampedPage,
    perPage,
    total,
    totalPages,
  };
}

const ITEMS_PER_PAGE = 20;

export function InvocationHistorySidebar({
  onSelectResult,
}: InvocationHistorySidebarProps) {
  const [isOpen, setIsOpen] = useState(true);
  const [currentPage, setCurrentPage] = useState(1);
  const { history, clearHistory } = useInvocationHistory();

  const pagination = paginate(history, currentPage, ITEMS_PER_PAGE);

  // Clamp the selected page when history shrinks or is cleared.
  useEffect(() => {
    if (currentPage !== pagination.page) {
      setCurrentPage(pagination.page);
    }
  }, [currentPage, pagination.page]);

  // Keep the page valid after entries are removed.
  const visibleHistory = pagination.items;

  {history.length > 0 && (
  <div
    style={{
      display: 'flex',
      alignItems: 'center',
      justifyContent: 'space-between',
      gap: '8px',
      padding: '10px 12px',
      borderTop: '1px solid #30363d',
      flexShrink: 0,
    }}
  >
    <button
      type="button"
      onClick={() => setCurrentPage((page) => Math.max(1, page - 1))}
      disabled={pagination.page <= 1}
      aria-label="Previous history page"
      style={{
        display: 'flex',
        alignItems: 'center',
        gap: '4px',
        padding: '5px 7px',
        border: '1px solid #30363d',
        borderRadius: '5px',
        background: 'transparent',
        color: pagination.page <= 1 ? '#484f58' : 'var(--text-primary)',
        cursor: pagination.page <= 1 ? 'not-allowed' : 'pointer',
        opacity: pagination.page <= 1 ? 0.6 : 1,
      }}
    >
      <ChevronLeft size={14} />
      <span style={{ fontSize: '11px' }}>Previous</span>
    </button>

    <span
      aria-live="polite"
      style={{
        fontSize: '11px',
        color: 'var(--text-secondary)',
        whiteSpace: 'nowrap',
      }}
    >
      Page {pagination.page} of {pagination.totalPages}
      <br />
      {pagination.total} total
    </span>

    <button
      type="button"
      onClick={() =>
        setCurrentPage((page) =>
          Math.min(pagination.totalPages, page + 1),
        )
      }
      disabled={pagination.page >= pagination.totalPages}
      aria-label="Next history page"
      style={{
        display: 'flex',
        alignItems: 'center',
        gap: '4px',
        padding: '5px 7px',
        border: '1px solid #30363d',
        borderRadius: '5px',
        background: 'transparent',
        color:
          pagination.page >= pagination.totalPages
            ? '#484f58'
            : 'var(--text-primary)',
        cursor:
          pagination.page >= pagination.totalPages
            ? 'not-allowed'
            : 'pointer',
        opacity: pagination.page >= pagination.totalPages ? 0.6 : 1,
      }}
    >
      <span style={{ fontSize: '11px' }}>Next</span>
      <ChevronRight size={14} />
    </button>
  </div>
)}