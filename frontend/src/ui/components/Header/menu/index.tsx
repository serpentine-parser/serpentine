import { useEffect } from "react";
import { LayoutSettingsPanel, LayoutSettingsPanelProps } from "./LayoutSettings";
import { ThemeToggle } from "./ThemeToggle";
import type { Theme } from "@ui/lib/ThemeContext";

interface AccountMenuProps extends LayoutSettingsPanelProps {
  isOpen: boolean;
  onClose: () => void;
  theme: Theme;
  setTheme: (theme: Theme) => void;
}

export default function AccountMenu({
  isOpen,
  onClose,
  theme,
  setTheme,
  layoutSettings,
  setLayoutSettings,
  flipLayoutDirection,
  expandAll,
  collapseAll,
}: AccountMenuProps) {
  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, [onClose]);

  if (!isOpen) return null;

  return (
    <div
      className="absolute right-0 top-full mt-2 w-72 bg-white dark:bg-slate-800 border border-gray-200 dark:border-slate-700 rounded shadow-lg z-50"
    >
      <div className="p-3">
        <div className="space-y-2">
          <ThemeToggle theme={theme} setTheme={setTheme} />
          <LayoutSettingsPanel
            layoutSettings={layoutSettings}
            setLayoutSettings={setLayoutSettings}
            flipLayoutDirection={flipLayoutDirection}
            expandAll={expandAll}
            collapseAll={collapseAll}
          />
        </div>
      </div>
    </div>
  );
}
