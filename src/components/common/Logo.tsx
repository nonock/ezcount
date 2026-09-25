import { cn } from "@/lib/utils";
import type React from "react";

/** The app icon: shares that add up to the total. Size it with `className` (e.g. `size-8`). */
export const LogoMark: React.FC<{ className?: string }> = ({ className }) => (
  <svg viewBox="0 0 100 100" aria-hidden="true" className={cn("shrink-0", className)}>
    <rect width="100" height="100" rx="22" fill="#4f39f6" />
    <rect x="20" y="31" width="17" height="13" rx="6.5" fill="#ffffff" />
    <rect x="42" y="31" width="13" height="13" rx="6.5" fill="#ffffff" />
    <rect x="60" y="31" width="20" height="13" rx="6.5" fill="#7ee2a8" />
    <rect x="20" y="56" width="60" height="13" rx="6.5" fill="#ffffff" />
  </svg>
);

/** "ezcount", with "ez" in the brand color. */
export const Wordmark: React.FC<{ className?: string }> = ({ className }) => (
  <span className={cn("font-semibold tracking-tight", className)}>
    <span className="text-[#4f39f6] dark:text-[#9b8cff]">ez</span>count
  </span>
);
