import { afterEach, describe, expect, it, vi } from "vitest";
import { i18n } from "@/lib/i18n/index.svelte";
import {
  amountInput,
  currencySymbol,
  expenseTitle,
  formatDate,
  formatDateGroupHeader,
  formatDateInput,
  formatDateTime,
  formatList,
  formatMoney,
  getLocalDateKey,
  moneyParts,
  serverName,
} from "./formatters";

/** French puts no-break spaces in amounts: plain ones read better in a test. */
const plain = (text: string) => text.replace(/\s/g, " ");

afterEach(() => {
  i18n.choose("en");
  vi.useRealTimers();
});

describe("moneyParts", () => {
  it("separates the cents from the rest", () => {
    expect(moneyParts(123450, "EUR")).toEqual({ before: "€1,234", cents: ".50", after: "" });
  });

  it("leaves the cents out of a round amount", () => {
    expect(moneyParts(9000, "EUR")).toEqual({ before: "€90", cents: "", after: "" });
  });

  it("signs what is above zero when asked", () => {
    expect(moneyParts(1250, "EUR", true).before).toBe("+€12");
    expect(moneyParts(-1250, "EUR", true).before).toBe("-€12");
    expect(moneyParts(0, "EUR", true).before).toBe("€0");
  });

  it("puts the sign after the amount in French", () => {
    i18n.choose("fr");
    const parts = moneyParts(123450, "EUR");
    expect(plain(parts.before)).toBe("1 234");
    expect(parts.cents).toBe(",50");
    expect(plain(parts.after)).toBe(" €");
  });

  it("adds the code of what isn't a currency", () => {
    expect(moneyParts(1250, "TOKEN")).toEqual({ before: "12", cents: ".50", after: " TOKEN" });
    expect(moneyParts(1200, "TOKEN")).toEqual({ before: "12", cents: "", after: " TOKEN" });
  });
});

describe("formatMoney", () => {
  it("writes an amount as the language does", () => {
    expect(formatMoney(1250, "USD")).toBe("$12.50");
    expect(formatMoney(-500)).toBe("-€5");
    i18n.choose("fr");
    expect(plain(formatMoney(1250, "EUR"))).toBe("12,50 €");
  });
});

describe("currencySymbol", () => {
  it("is the sign, or the code where there is none", () => {
    expect(currencySymbol("EUR")).toBe("€");
    expect(currencySymbol("USD")).toBe("$");
    expect(currencySymbol("CHF")).toBe("CHF");
    expect(currencySymbol("TOKEN")).toBe("TOKEN");
  });
});

describe("amountInput", () => {
  it("writes an amount as it is typed", () => {
    expect(amountInput(1250)).toBe("12.50");
    expect(amountInput(1205)).toBe("12.05");
    expect(amountInput(9000)).toBe("90");
    expect(amountInput(0)).toBe("0");
  });
});

describe("formatList", () => {
  it("lists names the way the language does", () => {
    expect(formatList(["Alice", "Bob", "Carol"])).toBe("Alice, Bob, and Carol");
    expect(formatList(["Alice"])).toBe("Alice");
    i18n.choose("fr");
    expect(formatList(["Alice", "Bob", "Carol"])).toBe("Alice, Bob et Carol");
  });
});

describe("dates", () => {
  const noon = new Date(2026, 2, 15, 12, 30);

  it("writes a day", () => {
    expect(formatDate(noon.toISOString())).toBe("Mar 15, 2026");
    i18n.choose("fr");
    expect(formatDate(noon.toISOString())).toBe("15 mars 2026");
  });

  it("writes a day and its time", () => {
    expect(formatDateTime(noon.toISOString())).toMatch(/^Mar 15, 2026.*12:30/);
  });

  it("returns what isn't a date as it came", () => {
    expect(formatDate("soon")).toBe("soon");
    expect(formatDateTime("soon")).toBe("soon");
    expect(formatDateGroupHeader("soon")).toBe("soon");
    expect(formatDateInput("soon")).toBe("");
  });

  it("gives the local day as a date field takes it", () => {
    expect(formatDateInput(new Date(2026, 0, 5, 23, 59))).toBe("2026-01-05");
    expect(getLocalDateKey(noon.toISOString())).toBe("2026-03-15");
  });

  it("heads a day's expenses with today, yesterday or the date", () => {
    vi.useFakeTimers({ now: noon, toFake: ["Date"] });
    expect(formatDateGroupHeader(new Date(2026, 2, 15, 8).toISOString())).toBe("Today");
    expect(formatDateGroupHeader(new Date(2026, 2, 14, 23).toISOString())).toBe("Yesterday");
    expect(formatDateGroupHeader(new Date(2026, 2, 10, 8).toISOString())).toBe("Tue, Mar 10");
    expect(formatDateGroupHeader(new Date(2025, 11, 31, 8).toISOString())).toBe(
      "Wed, Dec 31, 2025"
    );
  });
});

describe("expenseTitle", () => {
  it("shows the core's payment title in the app's language", () => {
    const payment = { title: "Payment: Ann → Bob", is_reimbursement: true };
    expect(expenseTitle(payment)).toBe("Payment: Ann → Bob");
    i18n.choose("fr");
    expect(expenseTitle(payment)).toBe("Paiement : Ann → Bob");
  });

  it("leaves a title someone typed alone", () => {
    i18n.choose("fr");
    expect(expenseTitle({ title: "Payment: rent", is_reimbursement: false })).toBe("Payment: rent");
    expect(expenseTitle({ title: "Thanks", is_reimbursement: true })).toBe("Thanks");
  });
});

describe("serverName", () => {
  it("keeps the host of a relay's address", () => {
    expect(serverName("https://relay.example.com")).toBe("relay.example.com");
    expect(serverName("http://localhost:8787/")).toBe("localhost:8787");
    expect(serverName("not a url")).toBe("not a url");
  });
});
