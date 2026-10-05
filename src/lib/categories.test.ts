import { afterEach, describe, expect, it } from "vitest";
import { CATEGORIES, categoryName, categoryOf, categoryTone } from "./categories";
import { i18n } from "./i18n/index.svelte";

afterEach(() => i18n.choose("en"));

describe("categoryOf", () => {
  it("finds a category by its key", () => {
    expect(categoryOf("food")?.key).toBe("food");
  });

  it("is nothing for an expense without one", () => {
    expect(categoryOf(null)).toBeNull();
    expect(categoryOf(undefined)).toBeNull();
    expect(categoryOf("")).toBeNull();
  });

  it("files a key from a newer version under Other", () => {
    expect(categoryOf("spaceships")?.key).toBe("other");
  });
});

describe("categoryName", () => {
  it("names the category in the app's language", () => {
    expect(categoryName("food")).toBe("Restaurants");
    expect(categoryName(null)).toBe("No category");
    i18n.choose("fr");
    expect(categoryName("transport")).toBe("Transport");
    expect(categoryName(null)).toBe("Sans catégorie");
  });
});

describe("categoryTone", () => {
  it("gives each category a color, and none without a category", () => {
    expect(categoryTone("food")).toContain("tone-0");
    expect(categoryTone("groceries")).toContain("tone-1");
    expect(categoryTone(null)).toBe("");
  });

  it("has unique keys", () => {
    const keys = CATEGORIES.map((c) => c.key);
    expect(new Set(keys).size).toBe(keys.length);
  });
});
