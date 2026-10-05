// Repeated expenses and the trash, kept up when a group is read (as `api::keep_up` does).

import type { MockGroup } from "./types";

// The day of occurrence `n` of a repeated expense, like doc.rs's `occurrence`: the day of
// the month is kept, or the month's last.
export function occurrence(start: string, every: string, n: number) {
  const date = new Date(start);
  if (every === "week") {
    date.setUTCDate(date.getUTCDate() + 7 * n);
    return date.toISOString();
  }
  const day = date.getUTCDate();
  date.setUTCDate(1);
  date.setUTCMonth(date.getUTCMonth() + (every === "year" ? 12 * n : n));
  const last = new Date(Date.UTC(date.getUTCFullYear(), date.getUTCMonth() + 1, 0)).getUTCDate();
  date.setUTCDate(Math.min(day, last));
  return date.toISOString();
}

// Adds the repeated expenses whose day has come, like doc.rs's `add_due_expenses`.
export function addDueExpenses(group: MockGroup) {
  const now = new Date().toISOString();
  const member = (id: string) => group.participants.some((p) => p.id === id && !p.removed);
  for (const model of group.recurring ?? []) {
    model.made ??= 0;
    model.next = occurrence(model.start, model.every, model.made);
    model.paused = ![
      model.paid_by,
      ...(model.payers ?? []).map((p) => p.participant_id),
      ...model.splits.map((s) => s.participant_id),
    ].every(member);
    while (!model.paused && model.next <= now) {
      const id = `${model.id}-${model.next.slice(0, 10).replaceAll("-", "")}`;
      const taken = [...group.expenses, ...(group.trash ?? []).map((d) => d.expense)];
      if (!taken.some((e) => e.id === id)) {
        group.expenses.unshift({
          id,
          group_id: group.id,
          title: model.title,
          category: model.category ?? null,
          amount_cents: model.amount_cents,
          income: model.income ?? false,
          paid_by: model.paid_by,
          payers: model.payers ?? [],
          splits: model.splits,
          created_at: model.next,
          updated_at: model.next,
          history: [],
          is_reimbursement: false,
          added_at: model.next,
          added_by: model.added_by ?? null,
          recurring: model.id,
        });
      }
      model.made += 1;
      model.next = occurrence(model.start, model.every, model.made);
    }
  }
}

// What reading a group does first, like api.rs's `keep_up`: the repeated expenses due are
// added, and what was deleted more than 30 days ago leaves the trash.
export function keepUp(group: MockGroup) {
  addDueExpenses(group);
  const kept = new Date(Date.now() - 30 * 24 * 3600 * 1000).toISOString();
  group.trash = (group.trash ?? []).filter((d) => d.deleted_at > kept);
}
