import { fireEvent, render, screen } from "@testing-library/svelte";
import { afterEach, describe, expect, it } from "vitest";
import { i18n, t } from "@/lib/i18n/index.svelte";
import StorageWarnings from "./StorageWarnings.svelte";

afterEach(() => i18n.choose("en"));

describe("StorageWarnings", () => {
  it("shows nothing when everything was read", () => {
    const { container } = render(StorageWarnings, { warnings: [] });
    expect(container.textContent?.trim()).toBe("");
  });

  it("lists what couldn't be read, and says it was kept", () => {
    const { container } = render(StorageWarnings, {
      warnings: ["Group g1 could not be loaded (bad data). Its data was kept in the database."],
    });
    expect(screen.getAllByRole("listitem")).toHaveLength(1);
    expect(container.textContent).toContain("Group g1 could not be loaded");
    expect(container.textContent).toContain(t("app.storageKept"));
  });

  it("says it in the app's language", () => {
    i18n.choose("fr");
    const { container } = render(StorageWarnings, {
      warnings: ["Group g1 could not be loaded (bad data). Its data was kept in the database."],
    });
    expect(container.textContent).toContain("Le groupe g1 n'a pas pu être chargé");
  });

  it("goes away once dismissed", async () => {
    const { container } = render(StorageWarnings, { warnings: ["Something is unreadable"] });
    await fireEvent.click(screen.getByRole("button", { name: t("common.dismiss") }));
    expect(container.textContent?.trim()).toBe("");
  });
});
