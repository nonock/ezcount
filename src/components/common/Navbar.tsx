import type React from "react";
import type { Group } from "../../types";

interface NavbarProps {
  currentGroup: Group | null;
  onNavigateHome: () => void;
  onOpenCreateGroup: () => void;
}

export const Navbar: React.FC<NavbarProps> = ({
  currentGroup,
  onNavigateHome,
  onOpenCreateGroup,
}) => {
  return (
    <header className="sticky top-0 z-30 border-b border-slate-800/80 bg-slate-950/75 backdrop-blur-xl">
      <div className="max-w-6xl mx-auto px-4 h-16 flex items-center justify-between">
        {/* Brand & Breadcrumb */}
        <div className="flex items-center gap-3">
          <button
            type="button"
            onClick={onNavigateHome}
            className="flex items-center gap-2.5 text-left group cursor-pointer rounded-xl p-1 -m-1 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-500"
          >
            <div className="w-9 h-9 rounded-xl bg-gradient-to-tr from-indigo-600 via-indigo-500 to-emerald-400 p-[1.5px] shadow-lg shadow-indigo-500/20 group-hover:scale-105 transition-transform duration-200">
              <div className="w-full h-full bg-slate-950 rounded-[10px] flex items-center justify-center">
                <span className="text-base font-black bg-gradient-to-r from-indigo-400 to-emerald-400 bg-clip-text text-transparent">
                  ez
                </span>
              </div>
            </div>
            <span className="text-lg font-extrabold tracking-tight text-white group-hover:text-indigo-200 transition-colors duration-150">
              ezcount
            </span>
          </button>

          {currentGroup && (
            <div className="flex items-center gap-2 text-xs">
              <span className="text-slate-600" aria-hidden="true">
                /
              </span>
              <span className="text-slate-300 font-semibold px-2 py-0.5 rounded-md bg-slate-800/80 border border-slate-700/60 max-w-[160px] sm:max-w-[240px] truncate">
                {currentGroup.name}
              </span>
            </div>
          )}
        </div>

        {/* Global Actions */}
        <div className="flex items-center gap-2 sm:gap-3">
          <button
            type="button"
            onClick={onOpenCreateGroup}
            className="inline-flex items-center gap-1.5 px-3.5 py-1.5 rounded-xl text-xs sm:text-sm font-semibold bg-indigo-600 hover:bg-indigo-500 text-white shadow-lg shadow-indigo-600/25 hover:shadow-indigo-500/40 transition-all duration-150 active:scale-95 cursor-pointer touch-manipulation focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-500 focus-visible:ring-offset-2 focus-visible:ring-offset-slate-950"
          >
            <svg
              className="w-4 h-4"
              fill="none"
              viewBox="0 0 24 24"
              stroke="currentColor"
              strokeWidth="2.5"
              aria-hidden="true"
            >
              <path strokeLinecap="round" strokeLinejoin="round" d="M12 4.5v15m7.5-7.5h-15" />
            </svg>
            <span>New Group</span>
          </button>
        </div>
      </div>
    </header>
  );
};
