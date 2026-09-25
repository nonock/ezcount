import { cn } from "@/lib/utils";
import type { PasswordStrength } from "@/types";
import type React from "react";

// By zxcvbn score, 0 to 4. Sign-up needs 3.
const LEVELS = [
  { label: "Too weak", color: "bg-negative" },
  { label: "Too weak", color: "bg-negative" },
  { label: "Weak", color: "bg-amber-500" },
  { label: "Good", color: "bg-positive" },
  { label: "Strong", color: "bg-positive" },
];

/**
 * A four-part bar and a short verdict for a new password, with what to change while it is
 * too weak. `id` goes on the verdict, for the password field's `aria-describedby`.
 */
export const PasswordStrengthMeter: React.FC<{ strength: PasswordStrength; id: string }> = ({
  strength,
  id,
}) => {
  const level = LEVELS[strength.score] ?? LEVELS[0];
  // Strong enough but under the minimum length: only length is missing.
  const label = !strength.acceptable && strength.score >= 3 ? "Too short" : level.label;
  const color = strength.acceptable
    ? level.color
    : strength.score >= 3
      ? "bg-amber-500"
      : level.color;
  const hint = strength.acceptable ? null : (strength.warning ?? strength.suggestions[0] ?? null);

  return (
    <div className="space-y-1.5">
      <div className="grid grid-cols-4 gap-1" aria-hidden>
        {[1, 2, 3, 4].map((segment) => (
          <div
            key={segment}
            className={cn(
              "h-1.5 rounded-full transition-colors",
              segment <= Math.max(1, strength.score) ? color : "bg-muted"
            )}
          />
        ))}
      </div>
      <p id={id} aria-live="polite" className="text-sm text-muted-foreground">
        <span className="font-medium text-foreground">{label}</span>
        {hint && <> · {hint}</>}
      </p>
    </div>
  );
};
