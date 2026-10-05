import { describe, expect, it } from "vitest";
import { askConfirm, confirmation } from "./confirm.svelte";

describe("askConfirm", () => {
  it("opens the dialog and resolves with the user's answer", async () => {
    const answer = askConfirm({ title: "Delete?", destructive: true });
    expect(confirmation.open).toBe(true);
    expect(confirmation.options).toEqual({ title: "Delete?", destructive: true });
    confirmation.settle(true);
    await expect(answer).resolves.toBe(true);
    expect(confirmation.open).toBe(false);
  });

  it("keeps the text while the dialog closes", () => {
    askConfirm({ title: "Leave?" });
    confirmation.settle(false);
    expect(confirmation.options.title).toBe("Leave?");
  });

  it("answers no to a question another one replaces", async () => {
    const first = askConfirm({ title: "First?" });
    const second = askConfirm({ title: "Second?" });
    await expect(first).resolves.toBe(false);
    expect(confirmation.options.title).toBe("Second?");
    confirmation.settle(true);
    await expect(second).resolves.toBe(true);
  });

  it("ignores an answer when nothing was asked", () => {
    confirmation.settle(true);
    expect(confirmation.open).toBe(false);
  });
});
