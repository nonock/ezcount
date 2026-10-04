// What happened in a group, the latest first, worked out from what the group keeps: who added,
// edited, deleted and restored its expenses, and who came and went.

import type { Expense, Group } from "@/types";
import { expenseTitle, formatList, formatMoney } from "@/utils/formatters";
import { summaryText } from "./i18n/backend";
import { t } from "./i18n/index.svelte";

export type ActivityAction = "added" | "repeated" | "edited" | "deleted" | "restored" | "commented";

export interface ActivityEvent {
  key: string;
  at: string;
  about: "expense" | "member";
  action: ActivityAction | "joined" | "left" | "created";
  text: string;
  /** The amount, or what an edit changed. */
  detail?: string;
}

/** The summary the core gives an expense put back from the trash. */
const RESTORED = "Restored from the trash";

export function groupActivity(group: Group): ActivityEvent[] {
  const nameOf = (id: string | null | undefined) =>
    id ? (group.participants.find((p) => p.id === id)?.name ?? null) : null;
  const events: ActivityEvent[] = [];

  /** "Alice added Taxi", or "Taxi was added" when the group doesn't know who did. */
  const said = (action: ActivityAction, by: string | null | undefined, title: string) => {
    const who = nameOf(by);
    if (action === "repeated") return t("activity.repeated", title);
    return who ? t(`activity.${action}By`, who, title) : t(`activity.${action}`, title);
  };

  const ofExpense = (e: Expense) => {
    const title = expenseTitle(e);
    const history = e.history ?? [];
    // From before additions were dated, an expense never edited was last touched when added.
    const addedAt = e.added_at ?? (history.length === 0 ? e.updated_at : null);
    if (addedAt) {
      // The first of a repeated expense was added by someone; the next ones by the app.
      const action = e.recurring && e.id.startsWith(e.recurring) ? "repeated" : "added";
      events.push({
        key: `${e.id}-added`,
        at: addedAt,
        about: "expense",
        action,
        text: said(action, e.added_by, title),
        // What it was added as: the amount before its first edit.
        detail: formatMoney(history[0]?.previous_amount_cents ?? e.amount_cents, group.currency),
      });
    }
    history.forEach((entry, i) => {
      const restored = entry.summary === RESTORED;
      const action = restored ? "restored" : "edited";
      events.push({
        key: `${e.id}-edit-${i}`,
        at: entry.edited_at,
        about: "expense",
        action,
        text: said(action, entry.edited_by, title),
        detail: restored ? undefined : summaryText(entry.summary),
      });
    });
    for (const comment of e.comments ?? []) {
      events.push({
        key: `comment-${comment.id}`,
        at: comment.created_at,
        about: "expense",
        action: "commented",
        text: said("commented", comment.by, title),
        detail: comment.text,
      });
    }
  };

  group.expenses.forEach(ofExpense);
  for (const deleted of group.trash ?? []) {
    ofExpense(deleted.expense);
    events.push({
      key: `${deleted.expense.id}-deleted`,
      at: deleted.deleted_at,
      about: "expense",
      action: "deleted",
      text: said("deleted", deleted.deleted_by, expenseTitle(deleted.expense)),
      detail: formatMoney(deleted.expense.amount_cents, group.currency),
    });
  }

  for (const p of group.participants) {
    if (p.added_at) {
      const by = nameOf(p.added_by);
      events.push({
        key: `${p.id}-joined`,
        at: p.added_at,
        about: "member",
        action: "joined",
        text:
          p.added_by === p.id
            ? t("members.joined", p.name)
            : by
              ? t("members.addedBy", p.name, by)
              : t("members.added", p.name),
      });
    }
    if (p.removed && p.removed_at) {
      const by = nameOf(p.removed_by);
      events.push({
        key: `${p.id}-left`,
        at: p.removed_at,
        about: "member",
        action: "left",
        text: by ? t("members.removedBy", p.name, by) : t("members.removed", p.name),
      });
    }
  }

  // The members the group was made with, and those from before additions were dated.
  const founders = group.participants.filter((p) => !p.added_at).map((p) => p.name);
  if (founders.length > 0) {
    events.push({
      key: "created",
      at: group.created_at,
      about: "member",
      action: "created",
      text: t("members.founders", formatList(founders), founders.length),
    });
  }

  // The group's creation last, whatever the dates of what was imported into it.
  return events.sort((a, b) =>
    a.action === "created" ? 1 : b.action === "created" ? -1 : b.at.localeCompare(a.at)
  );
}
