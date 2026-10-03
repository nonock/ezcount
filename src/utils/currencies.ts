import { i18n } from "@/lib/i18n/index.svelte";

/** The currencies offered for a group. */
export const CURRENCIES = ["EUR", "USD", "GBP", "CHF", "CAD"];

const SYMBOLS: Record<string, string> = { EUR: "€", USD: "$", GBP: "£", CAD: "CA$" };

/** `EUR (€) — Euro`, with the currency's name in the app's language. */
export function currencyLabel(code: string): string {
  const symbol = SYMBOLS[code] ? ` (${SYMBOLS[code]})` : "";
  try {
    const name = new Intl.DisplayNames(i18n.locale, { type: "currency" }).of(code);
    if (!name || name === code) return `${code}${symbol}`;
    return `${code}${symbol} — ${name.charAt(0).toUpperCase()}${name.slice(1)}`;
  } catch {
    // Not a currency code the browser knows.
    return `${code}${symbol}`;
  }
}
