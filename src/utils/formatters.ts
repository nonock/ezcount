import { i18n, t } from "@/lib/i18n/index.svelte";

const moneyFormats = new Map<string, Intl.NumberFormat>();

/**
 * The number format for a currency in the app's language, `signed` with a + on what's above
 * zero, `whole` without decimals (for an amount that has no cents).
 */
function moneyFormat(currency: string, signed: boolean, whole: boolean): Intl.NumberFormat {
  const key = `${i18n.locale} ${currency} ${signed} ${whole}`;
  let format = moneyFormats.get(key);
  if (!format) {
    const sign = { signDisplay: signed ? "exceptZero" : "auto" } as const;
    const noDecimals = { minimumFractionDigits: 0, maximumFractionDigits: 0 };
    try {
      format = new Intl.NumberFormat(i18n.locale, {
        style: "currency",
        currency,
        ...sign,
        ...(whole ? noDecimals : {}),
      });
    } catch {
      // Not a currency code: the amount alone, the code is added by `moneyParts`.
      format = new Intl.NumberFormat(i18n.locale, {
        minimumFractionDigits: 2,
        maximumFractionDigits: 2,
        ...sign,
        ...(whole ? noDecimals : {}),
      });
    }
    moneyFormats.set(key, format);
  }
  return format;
}

/**
 * An amount as the user's language writes it (`€1,234.50`, `1 234,50 €`), in three pieces so
 * the cents can be shown smaller: what comes before them, the cents with their separator, and
 * what follows. A round amount has no cents: `€90`, not `€90.00`.
 */
export function moneyParts(
  cents: number,
  currency = "EUR",
  signed = false
): { before: string; cents: string; after: string } {
  const format = moneyFormat(currency, signed, cents % 100 === 0);
  const parts = format.formatToParts(cents / 100);
  const decimal = parts.findIndex((p) => p.type === "decimal");
  const text = (from: number, to?: number) =>
    parts
      .slice(from, to)
      .map((p) => p.value)
      .join("");
  const code = format.resolvedOptions().style === "currency" ? "" : ` ${currency}`;
  if (decimal < 0) return { before: text(0), cents: "", after: code };
  return {
    before: text(0, decimal),
    cents: text(decimal, decimal + 2),
    after: text(decimal + 2) + code,
  };
}

/** "€" for EUR, "$" for USD; the code itself where the language has no sign for it (CHF). */
export function currencySymbol(currency: string): string {
  try {
    const parts = new Intl.NumberFormat(i18n.locale, {
      style: "currency",
      currency,
      currencyDisplay: "narrowSymbol",
    }).formatToParts(0);
    return parts.find((p) => p.type === "currency")?.value ?? currency;
  } catch {
    return currency;
  }
}

/** An amount as typed in a form: "12.50", and "90" for a round one. */
export function amountInput(cents: number): string {
  return cents % 100 === 0 ? String(cents / 100) : (cents / 100).toFixed(2);
}

export function formatMoney(cents: number, currency = "EUR"): string {
  const parts = moneyParts(cents, currency);
  return parts.before + parts.cents + parts.after;
}

/** "Alice, Bob and Carol", the way the app's language lists things. */
export function formatList(items: string[]): string {
  try {
    return new Intl.ListFormat(i18n.locale, { style: "long", type: "conjunction" }).format(items);
  } catch {
    return items.join(", ");
  }
}

export function formatDate(isoString: string): string {
  try {
    const d = new Date(isoString);
    if (Number.isNaN(d.getTime())) return isoString;
    return d.toLocaleDateString(i18n.locale, {
      month: "short",
      day: "numeric",
      year: "numeric",
    });
  } catch {
    return isoString;
  }
}

export function formatDateTime(isoString: string): string {
  try {
    const d = new Date(isoString);
    if (Number.isNaN(d.getTime())) return isoString;
    return d.toLocaleString(i18n.locale, {
      month: "short",
      day: "numeric",
      year: "numeric",
      hour: "2-digit",
      minute: "2-digit",
    });
  } catch {
    return isoString;
  }
}

export function formatDateInput(date: Date | string = new Date()): string {
  const d = typeof date === "string" ? new Date(date) : date;
  if (Number.isNaN(d.getTime())) return "";
  const year = d.getFullYear();
  const month = String(d.getMonth() + 1).padStart(2, "0");
  const day = String(d.getDate()).padStart(2, "0");
  return `${year}-${month}-${day}`;
}

export function getLocalDateKey(isoString: string): string {
  return formatDateInput(isoString);
}

export function formatDateGroupHeader(isoString: string): string {
  try {
    const d = new Date(isoString);
    if (Number.isNaN(d.getTime())) return isoString;

    const now = new Date();
    const todayKey = formatDateInput(now);
    const yesterday = new Date(now);
    yesterday.setDate(yesterday.getDate() - 1);
    const yesterdayKey = formatDateInput(yesterday);

    const itemKey = formatDateInput(d);

    if (itemKey === todayKey) {
      return t("date.today");
    }
    if (itemKey === yesterdayKey) {
      return t("date.yesterday");
    }

    const isCurrentYear = d.getFullYear() === now.getFullYear();
    return d.toLocaleDateString(i18n.locale, {
      weekday: "short",
      month: "short",
      day: "numeric",
      ...(isCurrentYear ? {} : { year: "numeric" }),
    });
  } catch {
    return isoString;
  }
}

/**
 * An expense's title. A payment recorded without a note is titled by the core, in English
 * ("Payment: Ann → Bob"), so that part is shown in the app's language.
 */
export function expenseTitle(expense: {
  title: string;
  is_reimbursement?: boolean | null;
}): string {
  const prefix = "Payment: ";
  if (!expense.is_reimbursement || !expense.title.startsWith(prefix)) return expense.title;
  return t("expenses.paymentPrefix") + expense.title.slice(prefix.length);
}

/** `https://relay.example.com` → `relay.example.com`, for display. */
export function serverName(url: string): string {
  try {
    return new URL(url).host || url;
  } catch {
    return url;
  }
}
