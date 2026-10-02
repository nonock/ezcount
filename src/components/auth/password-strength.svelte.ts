import { api } from "@/services/api";
import type { PasswordStrength } from "@/types";

/**
 * Rates a new password as the user types, with the same check the app enforces. `current` is
 * null while `enabled` is false or the password is empty. Call it while a component
 * initializes.
 */
export function ratePassword(
  input: () => { password: string; username: string; enabled: boolean }
) {
  let strength = $state<PasswordStrength | null>(null);
  $effect(() => {
    const { password, username, enabled } = input();
    if (!enabled || !password) {
      strength = null;
      return;
    }
    let current = true;
    api
      .passwordStrength(password, username)
      .then((s) => {
        if (current) strength = s;
      })
      .catch((err) => console.error("Could not rate the password:", err));
    return () => {
      current = false;
    };
  });
  return {
    get current() {
      return strength;
    },
  };
}
