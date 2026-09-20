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
