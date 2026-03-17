import { useState } from "react";
import { createPortal } from "react-dom";
import LogoTitle from "./LogoTitle";
import AccountMenu from "./menu";
import { UserAvatar } from "./UserAvatar";
import type { Theme } from "@ui/lib/ThemeContext";
import type { LayoutSettings } from "@domains/graph/model/layoutTypes";

export interface HeaderProps {
  theme: Theme;
  setTheme: (theme: Theme) => void;
  layoutSettings: LayoutSettings;
  setLayoutSettings: (patch: Partial<LayoutSettings>) => void;
  flipLayoutDirection: () => void;
  expandAll: () => void;
  collapseAll: () => void;
}

export default function Header({
  theme,
  setTheme,
  layoutSettings,
  setLayoutSettings,
  flipLayoutDirection,
  expandAll,
  collapseAll,
}: HeaderProps) {
  const [menuOpen, setMenuOpen] = useState(false);

  return (
    <header className="sticky top-0 z-50 w-full shadow-sm border-b-4 px-4 py-2 transition-colors duration-200 bg-slate-100/95 backdrop-blur-sm border-emerald-500 dark:border-emerald-700 dark:bg-slate-900/95">
      <div className="w-full flex items-center justify-between h-12">
        <a href="/">
          <LogoTitle />
        </a>
        <div className="flex items-center gap-4">
          <div className="relative">
            {menuOpen && createPortal(
              <div className="fixed inset-0 z-40" onClick={() => setMenuOpen(false)} />,
              document.body
            )}
            <UserAvatar isAuthenticated={false} onClick={() => setMenuOpen(!menuOpen)} />
            {menuOpen && (
              <AccountMenu
                isOpen={menuOpen}
                onClose={() => setMenuOpen(false)}
                theme={theme}
                setTheme={setTheme}
                layoutSettings={layoutSettings}
                setLayoutSettings={setLayoutSettings}
                flipLayoutDirection={flipLayoutDirection}
                expandAll={expandAll}
                collapseAll={collapseAll}
              />
            )}
          </div>
        </div>
      </div>
    </header>
  );
}
