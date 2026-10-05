import { describe, expect, it } from "vitest";
import { formatIban, isIban, transferQrText } from "./sepa";

describe("formatIban", () => {
  it("groups by four without a trailing space", () => {
    expect(formatIban("FR7630006000011234567890189")).toBe("FR76 3000 6000 0112 3456 7890 189");
    expect(formatIban("DE89370400440532013000")).toBe("DE89 3704 0044 0532 0130 00");
    expect(formatIban("")).toBe("");
  });
});

describe("isIban", () => {
  it("accepts an IBAN whose check digits fit, however it is typed", () => {
    expect(isIban("FR7630006000011234567890189")).toBe(true);
    expect(isIban("de89 3704 0044 0532 0130 00")).toBe(true);
    expect(isIban("GB82 WEST 1234 5698 7654 32")).toBe(true);
  });

  it("refuses wrong check digits and what isn't shaped like an IBAN", () => {
    expect(isIban("FR7630006000011234567890188")).toBe(false);
    expect(isIban("DE00370400440532013000")).toBe(false);
    expect(isIban("FR76")).toBe(false);
    expect(isIban("")).toBe(false);
    expect(isIban("1234567890123456")).toBe(false);
  });
});

describe("transferQrText", () => {
  it("writes the EPC lines banking apps read", () => {
    const text = transferQrText({
      name: "Alice Martin",
      iban: "FR7630006000011234567890189",
      amountCents: 1250,
      reference: "Trip",
    });
    expect(text.split("\n")).toEqual([
      "BCD",
      "002",
      "1",
      "SCT",
      "",
      "Alice Martin",
      "FR7630006000011234567890189",
      "EUR12.50",
      "",
      "",
      "Trip",
    ]);
  });

  it("keeps what was typed on one line and within the format's lengths", () => {
    const lines = transferQrText({
      name: `  Alice\nMartin ${"x".repeat(100)}`,
      iban: "FR7630006000011234567890189",
      amountCents: 5,
      reference: "a\r\nb",
    }).split("\n");
    expect(lines).toHaveLength(11);
    expect(lines[5]).toHaveLength(70);
    expect(lines[5].startsWith("Alice Martin x")).toBe(true);
    expect(lines[7]).toBe("EUR0.05");
    expect(lines[10]).toBe("a b");
  });
});
