import { expect, vi } from "vitest";
import { confirmation } from "@/lib/state/confirm.svelte";

/** Answers the confirmation the app asks for, once it does. Returns what it asked. */
export async function answerConfirm(confirmed: boolean) {
  await vi.waitFor(() => expect(confirmation.open).toBe(true));
  const asked = confirmation.options;
  confirmation.settle(confirmed);
  return asked;
}
