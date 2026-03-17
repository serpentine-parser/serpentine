interface ExportButtonProps {
  onExportJson: () => void;
}

export function ExportButton({ onExportJson }: ExportButtonProps) {
  return (
    <button
      className="ml-4 px-4 py-2 bg-emerald-600 text-white rounded hover:bg-emerald-700 transition"
      onClick={onExportJson}
      title="Export current graph as JSON"
    >
      Export JSON
    </button>
  );
}
