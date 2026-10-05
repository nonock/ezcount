import { afterEach, describe, expect, it } from "vitest";
import { i18n } from "@/lib/i18n/index.svelte";
import { CURRENCIES, currencyLabel } from "./currencies";

afterEach(() => i18n.choose("en"));

describe("currencyLabel", () => {
  it("names a currency in the app's language, with its sign", () => {
    expect(currencyLabel("EUR")).toBe("EUR (€) — Euro");
    expect(currencyLabel("CHF")).toBe("CHF — Swiss Franc");
    i18n.choose("fr");
    expect(currencyLabel("USD")).toBe("USD ($) — Dollar des États-Unis");
  });

  it("shows the code alone for what the browser can't name", () => {
    expect(currencyLabel("not a code")).toBe("not a code");
  });

  it("labels every currency on offer", () => {
    for (const code of CURRENCIES) expect(currencyLabel(code)).toContain(" — ");
  });
});
