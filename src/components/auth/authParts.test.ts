import { fireEvent, render, screen } from "@testing-library/svelte";
import { afterEach, describe, expect, it } from "vitest";
import { i18n, t } from "@/lib/i18n/index.svelte";
import { DEFAULT_SERVER } from "@/lib/server";
import AuthServerField from "./AuthServerField.svelte";
import LanguageLinks from "./LanguageLinks.svelte";

afterEach(() => i18n.choose("system"));

describe("LanguageLinks", () => {
  it("offers each language under its own name, the current one pressed", () => {
    i18n.choose("en");
    render(LanguageLinks);
    expect(screen.getByRole("button", { name: "English" }).getAttribute("aria-pressed")).toBe(
      "true"
    );
    const french = screen.getByRole("button", { name: "Français" });
    expect(french.getAttribute("aria-pressed")).toBe("false");
    expect(french.getAttribute("lang")).toBe("fr");
  });

  it("changes the app's language", async () => {
    i18n.choose("en");
    render(LanguageLinks);
    await fireEvent.click(screen.getByRole("button", { name: "Français" }));
    expect(i18n.language).toBe("fr");
    expect(screen.getByRole("button", { name: "Français" }).getAttribute("aria-pressed")).toBe(
      "true"
    );
  });
});

describe("AuthServerField", () => {
  it("shows the relay and nothing to go back to while it is the default one", () => {
    i18n.choose("en");
    render(AuthServerField, { value: DEFAULT_SERVER });
    expect((screen.getByLabelText(t("common.server")) as HTMLInputElement).value).toBe(
      DEFAULT_SERVER
    );
    expect(screen.queryByRole("button", { name: t("auth.defaultServer") })).toBeNull();
  });

  it("offers to go back to the default relay from another one", async () => {
    i18n.choose("en");
    render(AuthServerField, { value: "https://relay.example.com" });
    await fireEvent.click(screen.getByRole("button", { name: t("auth.defaultServer") }));
    expect((screen.getByLabelText(t("common.server")) as HTMLInputElement).value).toBe(
      DEFAULT_SERVER
    );
    expect(screen.queryByRole("button", { name: t("auth.defaultServer") })).toBeNull();
  });
});
