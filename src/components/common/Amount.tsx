import { cn } from "@/lib/utils";
import { formatMoney } from "@/utils/formatters";
import type React from "react";

interface AmountProps {
  cents: number;
  currency: string;
  /** `balance` colors by sign (green = gets money back, red = owes) and shows a `+` sign. */
  tone?: "neutral" | "positive" | "balance";
  className?: string;
}

export const Amount: React.FC<AmountProps> = ({ cents, currency, tone = "neutral", className }) => {
  const color =
    tone === "positive" || (tone === "balance" && cents > 0)
      ? "text-positive"
      : tone === "balance" && cents < 0
        ? "text-negative"
        : undefined;
  return (
    <span className={cn("tabular-nums", color, className)}>
      {tone === "balance" && cents > 0 ? "+" : ""}
      {formatMoney(cents, currency)}
    </span>
  );
};
