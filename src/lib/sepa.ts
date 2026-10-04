// Bank transfers in euros: the QR code banking apps scan to fill one in.

/** An IBAN in groups of four, as banks print it. */
export function formatIban(iban: string): string {
  return iban.replace(/(.{4})(?=.)/g, "$1 ");
}

/** Whether the letters and digits typed make an IBAN: its two check digits must fit it. */
export function isIban(typed: string): boolean {
  const iban = typed.replace(/\s/g, "").toUpperCase();
  if (!/^[A-Z]{2}\d{2}[A-Z0-9]{11,30}$/.test(iban)) return false;
  let rest = 0;
  for (const c of iban.slice(4) + iban.slice(0, 4)) {
    const n = Number.parseInt(c, 36);
    rest = (n > 9 ? rest * 100 + n : rest * 10 + n) % 97;
  }
  return rest === 1;
}

/**
 * The text of the QR code of a SEPA credit transfer (the European Payments Council's
 * "Quick Response Code", version 2): a banking app that scans it has the recipient, the
 * account, the amount and the reference filled in. Euros only.
 */
export function transferQrText(transfer: {
  name: string;
  iban: string;
  amountCents: number;
  reference: string;
}): string {
  // One line each, so nothing typed may break a line.
  const line = (text: string, max: number) => text.replace(/\s+/g, " ").trim().slice(0, max);
  return [
    "BCD",
    "002",
    "1", // UTF-8
    "SCT",
    "", // The bank's code, which the IBAN gives within the SEPA area.
    line(transfer.name, 70),
    transfer.iban,
    `EUR${(transfer.amountCents / 100).toFixed(2)}`,
    "",
    "",
    line(transfer.reference, 140),
  ].join("\n");
}
