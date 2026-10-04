// A group's summary as a PDF file: its balances, who should pay whom, and its expenses. Loaded
// only when one is asked for, with the library and the fonts it needs.

import fontkit from "@pdf-lib/fontkit";
import fontUrl from "dejavu-fonts-ttf/ttf/DejaVuSans.ttf?url";
import boldFontUrl from "dejavu-fonts-ttf/ttf/DejaVuSans-Bold.ttf?url";
import { PDFDocument, type PDFFont, type PDFPage, rgb } from "pdf-lib";
import type { Group, ParticipantBalance, SettlementTransfer } from "@/types";
import { expenseTitle, formatDate, formatMoney } from "@/utils/formatters";
import { categoryName } from "./categories";
import { t } from "./i18n/index.svelte";
import { paidAmounts } from "./split";

// A4, in points.
const WIDTH = 595.28;
const HEIGHT = 841.89;
const MARGIN = 42;
const ROW = 17;

const INK = rgb(0.1, 0.11, 0.13);
const MUTED = rgb(0.42, 0.45, 0.5);
const RULE = rgb(0.86, 0.88, 0.9);
const GREEN = rgb(0.05, 0.5, 0.3);
const RED = rgb(0.75, 0.15, 0.15);

interface Column {
  title: string;
  /** Share of the page's width. */
  width: number;
  right?: boolean;
}

type Cell = string | { text: string; color?: ReturnType<typeof rgb>; bold?: boolean };

/** Dates and amounts come with spaces that don't break; a plain one is in every font. */
const plain = (text: string) => text.replace(/[   ]/g, " ");

export async function groupReport(
  group: Group,
  balances: ParticipantBalance[],
  settlements: SettlementTransfer[]
): Promise<Uint8Array> {
  const pdf = await PDFDocument.create();
  pdf.registerFontkit(fontkit);
  const load = async (url: string) => (await fetch(url)).arrayBuffer();
  const [regular, bold] = await Promise.all([
    pdf.embedFont(await load(fontUrl), { subset: true }),
    pdf.embedFont(await load(boldFontUrl), { subset: true }),
  ]);
  pdf.setTitle(group.name);
  pdf.setCreator("ezcount");

  let page: PDFPage = pdf.addPage([WIDTH, HEIGHT]);
  let y = HEIGHT - MARGIN;
  const inner = WIDTH - 2 * MARGIN;

  /** Moves down, onto a new page when `room` no longer fits on this one. */
  const need = (room: number) => {
    if (y - room >= MARGIN + 20) return;
    page = pdf.addPage([WIDTH, HEIGHT]);
    y = HEIGHT - MARGIN;
  };

  /** `text`, cut with an ellipsis when wider than `width`. */
  const fit = (text: string, font: PDFFont, size: number, width: number) => {
    let shown = plain(text);
    if (font.widthOfTextAtSize(shown, size) <= width) return shown;
    while (shown.length > 1 && font.widthOfTextAtSize(`${shown}…`, size) > width) {
      shown = shown.slice(0, -1);
    }
    return `${shown}…`;
  };

  const text = (
    content: string,
    x: number,
    options: { size?: number; font?: PDFFont; color?: ReturnType<typeof rgb>; width?: number } = {}
  ) => {
    const { size = 9.5, font = regular, color = INK, width = inner } = options;
    page.drawText(fit(content, font, size, width), { x, y, size, font, color });
  };

  const heading = (content: string) => {
    need(ROW * 3);
    y -= 26;
    text(content, MARGIN, { size: 12.5, font: bold });
    y -= 8;
  };

  const table = (columns: Column[], rows: Cell[][]) => {
    const total = columns.reduce((sum, c) => sum + c.width, 0);
    const widths = columns.map((c) => (c.width / total) * inner);
    const line = (cells: Cell[], header: boolean) => {
      need(ROW);
      y -= ROW;
      let x = MARGIN;
      cells.forEach((cell, i) => {
        const value = typeof cell === "string" ? { text: cell } : cell;
        const font = header || value.bold ? bold : regular;
        const size = header ? 8 : 9.5;
        const shown = fit(value.text, font, size, widths[i] - 8);
        const at = columns[i].right ? x + widths[i] - font.widthOfTextAtSize(shown, size) : x;
        page.drawText(shown, {
          x: at,
          y,
          size,
          font,
          color: header ? MUTED : (value.color ?? INK),
        });
        x += widths[i];
      });
      page.drawLine({
        start: { x: MARGIN, y: y - 5 },
        end: { x: MARGIN + inner, y: y - 5 },
        thickness: 0.5,
        color: RULE,
      });
    };
    line(
      columns.map((c) => c.title),
      true
    );
    for (const row of rows) line(row, false);
  };

  const money = (cents: number) => formatMoney(cents, group.currency);
  const nameOf = (id: string) =>
    group.participants.find((p) => p.id === id)?.name ?? t("common.unknown");
  const members = group.participants.filter((p) => !p.removed);

  // The group, and the day of the report.
  y -= 14;
  text(group.name, MARGIN, { size: 20, font: bold });
  y -= 16;
  text(
    `${t("report.title", formatDate(new Date().toISOString()))} · ${t("report.members", members.length)} · ${group.currency}`,
    MARGIN,
    { color: MUTED }
  );
  if (group.description) {
    y -= 14;
    text(group.description, MARGIN, { color: MUTED });
  }

  heading(t("report.balances"));
  table(
    [
      { title: t("report.member"), width: 4 },
      { title: t("report.paid"), width: 2, right: true },
      { title: t("report.share"), width: 2, right: true },
      { title: t("report.balance"), width: 2, right: true },
    ],
    balances.map((b) => [
      b.participant_name,
      money(b.paid_cents),
      money(b.owed_cents),
      {
        text: `${b.net_cents > 0 ? "+" : ""}${money(b.net_cents)}`,
        color: b.net_cents > 0 ? GREEN : b.net_cents < 0 ? RED : INK,
        bold: true,
      },
    ])
  );

  heading(t("settle.heading"));
  if (settlements.length === 0) {
    y -= ROW;
    text(t("settle.allSettled"), MARGIN, { color: MUTED });
  } else {
    table(
      [
        { title: t("report.who"), width: 6 },
        { title: t("common.amount"), width: 2, right: true },
      ],
      settlements.map((s) => [
        t("report.pays", s.from_name, s.to_name),
        { text: money(s.amount_cents), bold: true },
      ])
    );
  }

  // The oldest first, as a statement reads.
  const expenses = [...group.expenses].sort((a, b) => a.created_at.localeCompare(b.created_at));
  const spent = expenses
    .filter((e) => !e.is_reimbursement && !e.income)
    .reduce((sum, e) => sum + e.amount_cents, 0);
  heading(t("report.expenses", expenses.length));
  table(
    [
      { title: t("common.date"), width: 2 },
      { title: t("expense.description"), width: 5 },
      { title: t("common.paidBy"), width: 3 },
      { title: t("common.amount"), width: 2, right: true },
    ],
    expenses.map((e) => {
      const kind = e.is_reimbursement
        ? t("expenses.reimbursement")
        : e.income
          ? t("expenses.income")
          : e.category
            ? categoryName(e.category)
            : null;
      return [
        formatDate(e.created_at),
        kind ? `${expenseTitle(e)} (${kind})` : expenseTitle(e),
        paidAmounts(e)
          .map((p) => nameOf(p.id))
          .join(", "),
        {
          text: money(e.amount_cents),
          color: e.is_reimbursement || e.income ? GREEN : INK,
        },
      ];
    })
  );
  need(ROW);
  y -= ROW + 3;
  text(t("groups.totalSpent"), MARGIN, { font: bold });
  const total = plain(money(spent));
  page.drawText(total, {
    x: MARGIN + inner - bold.widthOfTextAtSize(total, 9.5),
    y,
    size: 9.5,
    font: bold,
    color: INK,
  });

  const pages = pdf.getPages();
  pages.forEach((sheet, i) => {
    const label = `ezcount · ${t("report.page", i + 1, pages.length)}`;
    sheet.drawText(label, {
      x: MARGIN + inner - regular.widthOfTextAtSize(label, 8),
      y: MARGIN - 14,
      size: 8,
      font: regular,
      color: MUTED,
    });
  });

  return pdf.save();
}
