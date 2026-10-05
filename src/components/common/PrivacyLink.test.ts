import { openUrl } from "@tauri-apps/plugin-opener";
import { fireEvent, render, screen } from "@testing-library/svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { i18n, t } from "@/lib/i18n/index.svelte";
import PrivacyLink from "./PrivacyLink.svelte";

vi.mock("@tauri-apps/plugin-opener", () => ({ openUrl: vi.fn() }));

beforeEach(() => {
  vi.mocked(openUrl).mockResolvedValue();
});

afterEach(() => i18n.choose("system"));

describe("PrivacyLink", () => {
  it("leads to the relay's own page, in the app's language", () => {
    i18n.choose("fr");
    render(PrivacyLink, { serverUrl: "https://relay.example.com" });
    const link = screen.getByRole("link", { name: t("menu.privacy") });
    expect(link.getAttribute("href")).toBe("https://relay.example.com/privacy?lang=fr");
  });

  it("opens it in the device's browser, not in the app", async () => {
    i18n.choose("en");
    render(PrivacyLink, { serverUrl: "https://relay.example.com" });
    const followed = await fireEvent.click(screen.getByRole("link", { name: t("menu.privacy") }));
    expect(openUrl).toHaveBeenCalledWith("https://relay.example.com/privacy?lang=en");
    // The click went no further: the web view stays on the app.
    expect(followed).toBe(false);
  });
});
