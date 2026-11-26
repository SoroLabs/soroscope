import React from 'react';

/**
 * Skeleton placeholder for the ResultViewer component.
 * Mirrors the final layout dimensions to avoid layout shift (CLS)
 * while async simulation results are being fetched.
 */
const ResultViewerSkeleton: React.FC = () => {
  return (
    <div
      className="result-viewer-skeleton"
      role="status"
      aria-busy="true"
      aria-live="polite"
      aria-label="Loading simulation results"
    >
      <div className="result-viewer-skeleton__header">
        <div className="skeleton-pulse skeleton-line skeleton-line--title" />
        <div className="skeleton-pulse skeleton-line skeleton-line--subtitle" />
      </div>

      <div className="result-viewer-skeleton__body">
        <div className="skeleton-pulse skeleton-block skeleton-block--chart" />

        <div className="result-viewer-skeleton__rows">
          {Array.from({ length: 5 }).map((_, index) => (
            <div key={index} className="result-viewer-skeleton__row">
              <div className="skeleton-pulse skeleton-line skeleton-line--label" />
              <div className="skeleton-pulse skeleton-line skeleton-line--value" />
            </div>
          ))}
        </div>
      </div>

      <style jsx>{`
        .result-viewer-skeleton {
          display: flex;
          flex-direction: column;
          gap: 1.5rem;
          width: 100%;
          padding: 1.5rem;
          box-sizing: border-box;
        }

        .result-viewer-skeleton__header {
          display: flex;
          flex-direction: column;
          gap: 0.5rem;
        }

        .result-viewer-skeleton__body {
          display: flex;
          flex-direction: column;
          gap: 1.5rem;
        }

        .result-viewer-skeleton__rows {
          display: flex;
          flex-direction: column;
          gap: 0.75rem;
        }

        .result-viewer-skeleton__row {
          display: flex;
          align-items: center;
          justify-content: space-between;
          gap: 1rem;
        }

        .skeleton-pulse {
          background: linear-gradient(
            90deg,
            rgba(148, 163, 184, 0.15) 25%,
            rgba(148, 163, 184, 0.3) 37%,
            rgba(148, 163, 184, 0.15) 63%
          );
          background-size: 400% 100%;
          border-radius: 6px;
          animation: skeleton-pulse 1.4s ease-in-out infinite;
        }

        .skeleton-line {
          height: 1rem;
        }

        .skeleton-line--title {
          width: 40%;
          height: 1.5rem;
        }

        .skeleton-line--subtitle {
          width: 60%;
        }

        .skeleton-line--label {
          width: 30%;
        }

        .skeleton-line--value {
          width: 20%;
        }

        .skeleton-block--chart {
          width: 100%;
          height: 240px;
          border-radius: 12px;
        }

        @keyframes skeleton-pulse {
          0% {
            background-position: 100% 50%;
          }
          100% {
            background-position: 0 50%;
          }
        }

        @media (prefers-reduced-motion: reduce) {
          .skeleton-pulse {
            animation: none;
          }
        }
      `}</style>
    </div>
  );
};

export default ResultViewerSkeleton;
