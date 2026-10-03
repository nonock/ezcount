import { backendText } from "@/lib/i18n/backend";

/** What went wrong, to show the user: the core's messages come in the app's language. */
export function errorMessage(err: unknown): string {
  return backendText(err instanceof Error ? err.message : String(err));
}
