// Expenses, payments, the trash, comments and repeated expenses.

import { checkSplits, itemsSplits, paidBy } from "../engine";
import { addDueExpenses } from "../recurring";
import { clone, findGroup, me, w } from "../state";
import type { Commands, MockExpense, MockGroup } from "../types";

/** The commands take the expense as one `expense` argument; this lays its fields out. */
function expenseArgs(args: any) {
  const e = args.expense;
  return {
    ...args,
    title: e.title,
    category: e.category || null,
    amountCents: e.amount_cents,
    paidBy: e.paid_by,
    payers: e.payers ?? [],
    splits: e.splits,
    createdAt: e.created_at ?? null,
    original: e.original ?? null,
    income: e.income ?? false,
    repeat: e.repeat || null,
    items: (e.items ?? []).map((item: any) => ({ ...item, name: item.name.trim() })),
  };
}

/** The expense of the group a command is about. */
function findExpense(group: MockGroup, expenseId: unknown): MockExpense {
  const expense = group.expenses.find((x) => x.id === expenseId);
  if (!expense) throw new Error("Expense not found");
  return expense;
}

export const expenseCommands: Commands = {
  suggest_exchange_rate: (args) => w.__RATES__?.[`${args.from}/${args.to}`] ?? null,

  add_expense(raw) {
    const args = expenseArgs(raw);
    const g = findGroup(args?.groupId);
    if (args.items.length > 0) {
      args.splits = itemsSplits(args.amountCents, args.original, args.items);
    }
    checkSplits(args.amountCents, args.original, args.splits);
    const who = paidBy(args);
    const now = new Date().toISOString();
    const createdAt = args?.createdAt || now;
    if (args.repeat && !["week", "month", "year"].includes(args.repeat)) {
      throw new Error("An expense repeats every week, month or year");
    }
    if (args.repeat && args.original) {
      throw new Error("A repeated expense has to be in the group's currency");
    }
    const recurring = args.repeat ? `rec-${Date.now()}` : null;
    const exp: MockExpense = {
      id: `exp-${Date.now()}`,
      group_id: args.groupId,
      title: args.title,
      category: args.category,
      amount_cents: args.amountCents,
      original: args.original ?? null,
      paid_by: who.paid_by,
      payers: who.payers,
      splits: args.splits,
      created_at: createdAt,
      updated_at: now,
      history: [],
      is_reimbursement: false,
      income: args.income,
      added_at: now,
      added_by: me(g),
      recurring,
      items: args.items,
      comments: [],
    };
    g.expenses.unshift(exp);
    if (recurring) {
      g.recurring ??= [];
      g.recurring.push({
        id: recurring,
        title: exp.title,
        category: exp.category,
        amount_cents: exp.amount_cents,
        income: exp.income,
        paid_by: exp.paid_by,
        payers: exp.payers,
        splits: exp.splits,
        every: args.repeat,
        start: createdAt,
        made: 1,
        added_by: me(g),
      });
      addDueExpenses(g);
    }
    return clone(g);
  },

  update_expense(raw) {
    const args = expenseArgs(raw);
    const g = findGroup(args?.groupId);
    const exp = findExpense(g, args?.expenseId);

    if (args.items.length > 0) {
      args.splits = itemsSplits(args.amountCents, args.original, args.items);
    }
    checkSplits(args.amountCents, args.original, args.splits);
    const who = paidBy(args);

    exp.history = exp.history || [];
    exp.history.push({
      edited_at: new Date().toISOString(),
      previous_title: exp.title,
      previous_category: exp.category ?? null,
      previous_amount_cents: exp.amount_cents,
      previous_paid_by: exp.paid_by,
      previous_payers: exp.payers ?? [],
      previous_splits: [...exp.splits],
      previous_original: exp.original ?? null,
      summary: `Amount changed to ${args.amountCents / 100} • Title updated to ${args.title}`,
      edited_by: me(g),
    });

    exp.title = args.title;
    exp.category = args.category;
    exp.amount_cents = args.amountCents;
    exp.original = args.original ?? null;
    exp.paid_by = who.paid_by;
    exp.payers = who.payers;
    exp.splits = args.splits;
    exp.items = args.items;
    if (args?.createdAt) {
      exp.created_at = args.createdAt;
    }
    exp.updated_at = new Date().toISOString();
    return clone(g);
  },

  delete_expense(args) {
    const g = findGroup(args?.groupId);
    const exp = findExpense(g, args?.expenseId);
    g.expenses = g.expenses.filter((x) => x !== exp);
    g.trash ??= [];
    g.trash.unshift({
      expense: exp,
      deleted_at: new Date().toISOString(),
      deleted_by: me(g),
    });
    return clone(g);
  },

  restore_expense(args) {
    const g = findGroup(args?.groupId);
    const deleted = g.trash?.find((d) => d.expense.id === args?.expenseId);
    if (!deleted) throw new Error("This expense is no longer in the trash");
    g.trash = g.trash?.filter((d) => d !== deleted);
    const exp = deleted.expense;
    const now = new Date().toISOString();
    exp.history = exp.history || [];
    exp.history.push({
      edited_at: now,
      previous_title: exp.title,
      previous_category: exp.category ?? null,
      previous_amount_cents: exp.amount_cents,
      previous_paid_by: exp.paid_by,
      previous_payers: exp.payers ?? [],
      previous_splits: [...exp.splits],
      previous_original: exp.original ?? null,
      summary: "Restored from the trash",
      edited_by: me(g),
    });
    exp.updated_at = now;
    g.expenses.unshift(exp);
    return clone(g);
  },

  purge_expense(args) {
    const g = findGroup(args?.groupId);
    if (!g.trash?.some((d) => d.expense.id === args?.expenseId)) {
      throw new Error("This expense is no longer in the trash");
    }
    g.trash = g.trash.filter((d) => d.expense.id !== args.expenseId);
    return clone(g);
  },

  add_expense_comment(args) {
    const g = findGroup(args?.groupId);
    const exp = findExpense(g, args?.expenseId);
    const text = String(args.text).trim();
    if (!text) throw new Error("A comment can't be empty");
    if (text.length > 500) {
      throw new Error("This comment is too long (500 characters at most)");
    }
    exp.comments ??= [];
    exp.comments.push({
      id: `com-${Date.now()}-${exp.comments.length}`,
      text,
      created_at: new Date().toISOString(),
      by: me(g),
    });
    return clone(g);
  },

  delete_expense_comment(args) {
    const g = findGroup(args?.groupId);
    const exp = g.expenses.find((x) => x.comments?.some((c) => c.id === args?.commentId));
    if (!exp) throw new Error("This comment no longer exists");
    exp.comments = exp.comments?.filter((c) => c.id !== args.commentId);
    return clone(g);
  },

  stop_recurring_expense(args) {
    const g = findGroup(args?.groupId);
    if (!g.recurring?.some((r) => r.id === args?.recurringId)) {
      throw new Error("This repeated expense no longer exists");
    }
    g.recurring = g.recurring.filter((r) => r.id !== args.recurringId);
    return clone(g);
  },

  record_reimbursement(args) {
    const g = findGroup(args?.groupId);
    const fromP = g.participants.find((p) => p.id === args.fromId)?.name || "Unknown";
    const toP = g.participants.find((p) => p.id === args.toId)?.name || "Unknown";
    const now = new Date().toISOString();
    g.expenses.unshift({
      id: `exp-${Date.now()}`,
      group_id: args.groupId,
      title: `Payment: ${fromP} → ${toP}`,
      amount_cents: args.amountCents,
      paid_by: args.fromId,
      splits: [{ participant_id: args.toId, shares: 1 }],
      created_at: now,
      updated_at: now,
      history: [],
      is_reimbursement: true,
      added_at: now,
      added_by: me(g),
    });
    return clone(g);
  },
};
