import { IconMoon, IconSun } from "@tabler/icons-react";
import type { Theme } from "@ui/lib/ThemeContext";

interface ThemeToggleProps {
  theme: Theme;
  setTheme: (theme: Theme) => void;
}

export const ThemeToggle = ({ theme, setTheme }: ThemeToggleProps) => {
  return (
    <button
      onClick={() => setTheme(theme === "dark" ? "light" : "dark")}
      className="flex items-center space-x-2 w-full px-2 py-2 text-sm text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-slate-700 rounded transition-colors duration-200 focus:outline-none focus:ring-2 focus:ring-emerald-500 focus:ring-offset-2 dark:focus:ring-offset-slate-800"
    >
      <span className="flex-shrink-0">
        {theme === "dark" ? <IconSun size={16} /> : <IconMoon size={16} />}
      </span>
      <span>Switch to {theme === "dark" ? "Light" : "Dark"} Mode</span>
    </button>
  );
};
