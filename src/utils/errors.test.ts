import { afterEach, describe, expect, it } from "vitest";
import { i18n } from "@/lib/i18n/index.svelte";
import { errorMessage } from "./errors";

afterEach(() => i18n.choose("en"));

describe("errorMessage", () => {
  it("reads an error or whatever was thrown", () => {
    expect(errorMessage(new Error("Group not found"))).toBe("Group not found");
    expect(errorMessage("Group not found")).toBe("Group not found");
    expect(errorMessage(42)).toBe("42");
  });

  it("gives the core's messages in the app's language", () => {
    i18n.choose("fr");
    expect(errorMessage(new Error("Group not found"))).toBe("Groupe introuvable");
  });
});
