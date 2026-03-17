interface LoadingOverlayProps {
  phase: "data" | "layout";
  nodeCount: number;
}

export function LoadingOverlay({ phase, nodeCount }: LoadingOverlayProps) {
  const message =
    phase === "data"
      ? "Fetching graph…"
      : `Computing layout for ${nodeCount.toLocaleString()} nodes…`;

  return (
    <div className="absolute inset-0 flex items-center justify-center bg-black/10 dark:bg-black/20 z-10">
      <div className="text-center p-6 bg-white/90 dark:bg-gray-800/90 rounded shadow flex items-center gap-3">
        <svg
          className="animate-spin h-5 w-5 text-gray-600 dark:text-gray-300 flex-shrink-0"
          xmlns="http://www.w3.org/2000/svg"
          fill="none"
          viewBox="0 0 24 24"
        >
          <circle
            className="opacity-25"
            cx="12"
            cy="12"
            r="10"
            stroke="currentColor"
            strokeWidth="4"
          />
          <path
            className="opacity-75"
            fill="currentColor"
            d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z"
          />
        </svg>
        <span className="font-semibold text-gray-900 dark:text-white">{message}</span>
      </div>
    </div>
  );
}
