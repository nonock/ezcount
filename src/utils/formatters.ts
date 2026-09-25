export function formatMoney(cents: number, currency = "EUR"): string {
  const abs = Math.abs(cents);
  const euros = (abs / 100).toFixed(2);
  const sign = cents < 0 ? "-" : "";

  const symbols: Record<string, string> = {
    EUR: "€",
    USD: "$",
    GBP: "£",
    CHF: "CHF",
    CAD: "CA$",
  };

  const sym = symbols[currency] || currency;
  if (sym === "$" || sym === "£" || sym === "CA$") {
    return `${sign}${sym}${euros}`;
  }
  return `${sign}${euros} ${sym}`;
}

export function formatDate(isoString: string): string {
  try {
    const d = new Date(isoString);
    if (Number.isNaN(d.getTime())) return isoString;
    return d.toLocaleDateString(undefined, {
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
    return d.toLocaleString(undefined, {
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
      return "Today";
    }
    if (itemKey === yesterdayKey) {
      return "Yesterday";
    }

    const isCurrentYear = d.getFullYear() === now.getFullYear();
    return d.toLocaleDateString(undefined, {
      weekday: "short",
      month: "short",
      day: "numeric",
      ...(isCurrentYear ? {} : { year: "numeric" }),
    });
  } catch {
    return isoString;
  }
}

/** `https://relay.example.com` → `relay.example.com`, for display. */
export function serverName(url: string): string {
  try {
    return new URL(url).host || url;
  } catch {
    return url;
  }
}
