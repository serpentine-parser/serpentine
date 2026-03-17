import { ReactNode } from "react";

interface ShellProps {
  children: ReactNode;
  sidebar: ReactNode;
  detailPanel?: ReactNode;
}

export function Shell({ children, sidebar, detailPanel }: ShellProps) {
  return (
    <div className="flex-1 min-h-0 flex flex-col transition-colors duration-200 bg-gray-50 dark:bg-gray-950 text-gray-900 dark:text-gray-100">
      <div className="flex-1 flex min-h-0">
        {sidebar}

        {detailPanel}

        <div className="flex-1 flex flex-col">
          <div className="flex-1 relative overflow-hidden">
            {children}
          </div>
        </div>
      </div>
    </div>
  );
}
