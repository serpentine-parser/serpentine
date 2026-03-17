import { IconUser } from "@tabler/icons-react";

interface UserAvatarProps {
  isAuthenticated: boolean;
  userName?: string;
  userEmail?: string;
  onClick: () => void;
}

const getInitials = (name?: string, email?: string): string => {
  if (name) {
    const nameParts = name.trim().split(" ");
    if (nameParts.length >= 2) return `${nameParts[0][0]}${nameParts[1][0]}`.toUpperCase();
    return nameParts[0][0].toUpperCase();
  }
  if (email) return email[0].toUpperCase();
  return "U";
};

export const UserAvatar = ({ isAuthenticated, userName, userEmail, onClick }: UserAvatarProps) => (
  <button
    aria-label="Open account menu"
    onClick={onClick}
    className="w-8 h-8 rounded-full bg-gray-100 dark:bg-slate-700 border dark:border-slate-600 flex items-center justify-center hover:bg-gray-200 dark:hover:bg-slate-600 transition-colors duration-200 focus:outline-none focus:ring-2 focus:ring-emerald-500 focus:ring-offset-2 dark:focus:ring-offset-slate-900"
  >
    {isAuthenticated ? (
      <div className="w-full h-full rounded-full bg-emerald-100 dark:bg-emerald-900/20 flex items-center justify-center">
        <span className="text-xs font-medium text-emerald-600 dark:text-emerald-400">
          {getInitials(userName, userEmail)}
        </span>
      </div>
    ) : (
      <IconUser size={18} strokeWidth={1.75} className="text-gray-500 dark:text-gray-200" />
    )}
  </button>
);
