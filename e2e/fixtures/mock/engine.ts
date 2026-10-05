// Who owes what, as the core works it out (`engine.rs`, and the checks of `doc/check.rs` the
// form can run into).

import type { MockExpense, MockExpenseSplit, MockGroup, MockOriginalAmount } from "./types";

const money = (cents: number) => (cents / 100).toFixed(2);

// `amount` in proportion to `weights`, like engine.rs's `split_weighted`.
export function splitWeighted(amount: number, weights: number[]) {
  const total = weights.reduce((sum, w) => sum + w, 0);
  if (total <= 0) return weights.map(() => 0);
  const allocated = weights.map((w) => ({
    base: Math.floor((amount * w) / total),
    rem: (amount * w) % total,
  }));
  const remainder = amount - allocated.reduce((sum, a) => sum + a.base, 0);
  const order = [...allocated.keys()].sort((a, b) => allocated[b].rem - allocated[a].rem);
  for (let i = 0; i < remainder; i++) {
    allocated[order[i]].base += 1;
  }
  return allocated.map((a) => a.base);
}

// What each person owes of an expense, like engine.rs's `owed`: fixed amounts first, the
// parts share the rest, all in the currency the expense was paid in.
export function splitAmount(exp: MockExpense) {
  const splits = exp.splits || [];
  const shares = splits.map((s) => s.shares);
  let owed: number[];
  if (splits.every((s) => s.fixed_cents == null)) {
    owed = splitWeighted(exp.amount_cents, shares);
  } else {
    const paid = exp.original?.amount_cents ?? exp.amount_cents;
    const fixed = splits.reduce((sum, s) => sum + (s.fixed_cents ?? 0), 0);
    const rest = splitWeighted(Math.max(paid - fixed, 0), shares);
    const there = splits.map((s, i) => s.fixed_cents ?? rest[i]);
    owed = exp.original ? splitWeighted(exp.amount_cents, there) : there;
  }
  return splits.map((s, i) => ({ pid: s.participant_id, base: owed[i] }));
}

// The checks of doc.rs's `check_amounts` that the form can run into.
export function checkSplits(
  amountCents: number,
  original: MockOriginalAmount | null,
  splits: MockExpenseSplit[]
) {
  const paid = original?.amount_cents ?? amountCents;
  const fixed = splits.reduce((sum, s) => sum + (s.fixed_cents ?? 0), 0);
  if (fixed > paid) {
    throw new Error(
      `The fixed amounts add up to ${money(fixed)}, more than the expense's ${money(paid)}`
    );
  }
  if (splits.every((s) => s.fixed_cents != null) && fixed !== paid) {
    throw new Error(`The amounts add up to ${money(fixed)}, not the expense's ${money(paid)}`);
  }
}

/** Who paid, as `PaidBy::new` and `check_payers` in doc.rs: who paid the most first. */
export function paidBy(args: any) {
  const payers = [...(args.payers ?? [])].sort((a, b) => b.amount_cents - a.amount_cents);
  if (payers.length < 2) {
    return { paid_by: payers[0]?.participant_id ?? args.paidBy, payers: [] };
  }
  if (payers.some((p) => p.amount_cents <= 0)) {
    throw new Error("What each payer paid must be above zero");
  }
  const total = payers.reduce((sum, p) => sum + p.amount_cents, 0);
  const paid = args.original?.amount_cents ?? args.amountCents;
  if (total !== paid) {
    throw new Error(
      `The payers paid ${money(total)} between them, not the expense's ${money(paid)}`
    );
  }
  return { paid_by: payers[0].participant_id, payers };
}

// The splits of an expense entered line by line, like doc.rs's `items_splits`.
export function itemsSplits(
  amountCents: number,
  original: MockOriginalAmount | null,
  items: any[]
) {
  const paid = original?.amount_cents ?? amountCents;
  const total = items.reduce((sum, item) => sum + item.amount_cents, 0);
  if (items.some((item) => item.participants.length === 0)) {
    throw new Error("Each item needs at least one person");
  }
  if (total !== paid) {
    throw new Error(`The items add up to ${money(total)}, not the expense's ${money(paid)}`);
  }
  const owed = new Map<string, number>();
  for (const item of items) {
    const people = item.participants.length;
    const each = Math.floor(item.amount_cents / people);
    const extra = item.amount_cents % people;
    item.participants.forEach((id: string, i: number) => {
      owed.set(id, (owed.get(id) ?? 0) + each + (i < extra ? 1 : 0));
    });
  }
  return [...owed]
    .filter(([, cents]) => cents > 0)
    .map(([participant_id, cents]) => ({ participant_id, shares: 0, fixed_cents: cents }));
}

export function computeBalances(group: MockGroup) {
  const map = new Map<string, { paid: number; owed: number }>();
  for (const p of group.participants) {
    map.set(p.id, { paid: 0, owed: 0 });
  }

  for (const exp of group.expenses) {
    // Money that came in counts the other way.
    const sign = exp.income ? -1 : 1;
    // Like `engine::paid`: several payers' amounts are in the currency paid.
    const payers = exp.payers ?? [];
    const there = payers.map((p) => p.amount_cents);
    const here = exp.original ? splitWeighted(exp.amount_cents, there) : there;
    const paid = payers.length
      ? payers.map((p, i) => ({ id: p.participant_id, cents: here[i] }))
      : [{ id: exp.paid_by, cents: exp.amount_cents }];
    for (const { id, cents } of paid) {
      const payer = map.get(id);
      if (payer) payer.paid += sign * cents;
    }

    for (const item of splitAmount(exp)) {
      const debtor = map.get(item.pid);
      if (debtor) debtor.owed += sign * item.base;
    }
  }

  // Mirrors engine.rs: removed participants only appear while they have something to settle.
  return group.participants
    .map((p) => {
      const data = map.get(p.id) || { paid: 0, owed: 0 };
      return {
        participant_id: p.id,
        participant_name: p.name,
        paid_cents: data.paid,
        owed_cents: data.owed,
        net_cents: data.paid - data.owed,
        removed: Boolean(p.removed),
      };
    })
    .filter((b) => !b.removed || b.net_cents !== 0);
}

export function computeSettlements(group: MockGroup) {
  const balances = computeBalances(group);
  const debtors = balances
    .filter((b) => b.net_cents < 0)
    .map((b) => ({ ...b, net_cents: -b.net_cents }))
    .sort((a, b) => b.net_cents - a.net_cents);

  const creditors = balances
    .filter((b) => b.net_cents > 0)
    .map((b) => ({ ...b }))
    .sort((a, b) => b.net_cents - a.net_cents);

  const transfers: any[] = [];
  let d = 0;
  let c = 0;
  while (d < debtors.length && c < creditors.length) {
    const debtor = debtors[d];
    const creditor = creditors[c];
    const payment = Math.min(debtor.net_cents, creditor.net_cents);
    if (payment > 0) {
      transfers.push({
        from_id: debtor.participant_id,
        from_name: debtor.participant_name,
        to_id: creditor.participant_id,
        to_name: creditor.participant_name,
        amount_cents: payment,
      });
      debtor.net_cents -= payment;
      creditor.net_cents -= payment;
    }
    if (debtor.net_cents === 0) d++;
    if (creditor.net_cents === 0) c++;
  }
  return transfers;
}
