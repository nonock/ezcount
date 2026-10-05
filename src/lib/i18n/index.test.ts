import { afterEach, describe, expect, it } from "vitest";
import { en } from "./en";
import { fr } from "./fr";
import { i18n, LANGUAGES, t } from "./index.svelte";

afterEach(() => i18n.choose("system"));

describe("t", () => {
  it("returns the text in the app's language", () => {
    i18n.choose("en");
    expect(t("date.today")).toBe("Today");
    i18n.choose("fr");
    expect(t("date.today")).toBe("Aujourd'hui");
  });

  it("gives a message its values", () => {
    i18n.choose("en");
    expect(t("activity.addedBy", "Alice", "Taxi")).toBe("Alice added Taxi");
  });
});

describe("i18n", () => {
  it("follows the device until a language is picked", () => {
    expect(i18n.choice).toBe("system");
    expect(i18n.language).toBe("en");
  });

  it("remembers the language picked and tells the page", () => {
    i18n.choose("fr");
    expect(i18n.language).toBe("fr");
    expect(localStorage.getItem("language")).toBe("fr");
    expect(document.documentElement.lang).toBe("fr");
    i18n.choose("system");
    expect(localStorage.getItem("language")).toBeNull();
  });

  it("formats in the device's variant of the language when it has one", () => {
    i18n.choose("en");
    expect(i18n.locale).toBe(navigator.language);
    i18n.choose("fr");
    expect(i18n.locale).toBe("fr");
  });
});

describe("messages", () => {
  it("offers every language it has messages for", () => {
    expect(LANGUAGES.map((l) => l.code)).toEqual(["en", "fr"]);
  });

  it("has every message in French, taking the same values", () => {
    const french = fr as Record<string, unknown>;
    for (const [key, message] of Object.entries(en)) {
      expect(typeof french[key], key).toBe(typeof message);
      if (typeof message === "function") {
        expect((french[key] as (...args: unknown[]) => string).length, key).toBe(message.length);
      }
    }
    expect(Object.keys(fr).sort()).toEqual(Object.keys(en).sort());
  });
});
