// In-memory Tauri IPC simulator for Playwright E2E tests
// Validates that all invocation signatures match the Rust tauri::command definitions.

export interface MockExpenseSplit {
  participant_id: string;
  shares: number;
}

export interface MockExpenseHistoryEntry {
  edited_at: string;
  previous_title: string;
  previous_amount_cents: number;
  previous_paid_by: string;
  previous_splits: MockExpenseSplit[];
  summary: string;
}

export interface MockExpense {
  id: string;
  group_id: string;
  title: string;
  amount_cents: number;
  paid_by: string;
  splits: MockExpenseSplit[];
  created_at: string;
  updated_at: string;
  history?: MockExpenseHistoryEntry[];
  is_reimbursement?: boolean;
}

export interface MockGroup {
  id: string;
  name: string;
  currency: string;
  participants: { id: string; name: string }[];
  expenses: MockExpense[];
  created_at: string;
}

export function installTauriMock() {
  const groups: MockGroup[] = [];

  function computeBalances(group: MockGroup) {
    const map = new Map<string, { paid: number; owed: number }>();
    for (const p of group.participants) {
      map.set(p.id, { paid: 0, owed: 0 });
    }

    for (const exp of group.expenses) {
      const payer = map.get(exp.paid_by);
      if (payer) payer.paid += exp.amount_cents;

      if (exp.splits && exp.splits.length > 0) {
        const totalShares = exp.splits.reduce((sum, s) => sum + s.shares, 0);
        if (totalShares > 0) {
          const allocated = exp.splits.map((s, idx) => {
            const base = Math.floor((exp.amount_cents * s.shares) / totalShares);
            const rem = (exp.amount_cents * s.shares) % totalShares;
            return { base, rem, idx, pid: s.participant_id };
          });

          const totalAllocated = allocated.reduce((sum, a) => sum + a.base, 0);
          const remainder = exp.amount_cents - totalAllocated;
          const order = [...allocated.keys()].sort((a, b) => allocated[b].rem - allocated[a].rem);
          for (let i = 0; i < remainder; i++) {
            allocated[order[i]].base += 1;
          }

          for (const item of allocated) {
            const debtor = map.get(item.pid);
            if (debtor) debtor.owed += item.base;
          }
        }
      }
    }

    return group.participants.map((p) => {
      const data = map.get(p.id) || { paid: 0, owed: 0 };
      return {
        participant_id: p.id,
        participant_name: p.name,
        paid_cents: data.paid,
        owed_cents: data.owed,
        net_cents: data.paid - data.owed,
      };
    });
  }

  function computeSettlements(group: MockGroup) {
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

  (window as any).__TAURI_INTERNALS__ = {
    invoke: async (cmd: string, args?: any) => {
      switch (cmd) {
        case "get_groups":
          return groups;

        case "get_group": {
          if (!args || typeof args.groupId !== "string") {
            throw new Error("missing required argument `group_id`");
          }
          const g = groups.find((x) => x.id === args.groupId);
          if (!g) throw new Error("Group not found");
          return g;
        }

        case "create_group": {
          if (!args || typeof args.name !== "string" || !Array.isArray(args.participants)) {
            throw new Error("invalid create_group arguments");
          }
          const now = new Date().toISOString();
          const newGroup: MockGroup = {
            id: `group-${Date.now()}`,
            name: args.name,
            currency: args.currency || "EUR",
            participants: args.participants.map((p: string, i: number) => ({
              id: `p-${i + 1}`,
              name: p,
            })),
            expenses: [],
            created_at: now,
          };
          groups.unshift(newGroup);
          return newGroup;
        }

        case "delete_group": {
          if (!args || typeof args.groupId !== "string") {
            throw new Error("missing required argument `group_id`");
          }
          const idx = groups.findIndex((x) => x.id === args.groupId);
          if (idx !== -1) groups.splice(idx, 1);
          return true;
        }

        case "add_participant": {
          const g = groups.find((x) => x.id === args?.groupId);
          if (!g) throw new Error("Group not found");
          g.participants.push({ id: `p-${Date.now()}`, name: args.name });
          return g;
        }

        case "add_expense": {
          const g = groups.find((x) => x.id === args?.groupId);
          if (!g) throw new Error("Group not found");
          const now = new Date().toISOString();
          const exp: MockExpense = {
            id: `exp-${Date.now()}`,
            group_id: args.groupId,
            title: args.title,
            amount_cents: args.amountCents,
            paid_by: args.paidBy,
            splits: args.splits,
            created_at: now,
            updated_at: now,
            history: [],
            is_reimbursement: false,
          };
          g.expenses.unshift(exp);
          return g;
        }

        case "update_expense": {
          const g = groups.find((x) => x.id === args?.groupId);
          if (!g) throw new Error("Group not found");
          const exp = g.expenses.find((x) => x.id === args?.expenseId);
          if (!exp) throw new Error("Expense not found");

          const prevTitle = exp.title;
          const prevAmount = exp.amount_cents;
          const prevPayer = exp.paid_by;
          const prevSplits = [...exp.splits];

          exp.history = exp.history || [];
          exp.history.push({
            edited_at: new Date().toISOString(),
            previous_title: prevTitle,
            previous_amount_cents: prevAmount,
            previous_paid_by: prevPayer,
            previous_splits: prevSplits,
            summary: `Amount changed to ${args.amountCents / 100} • Title updated to ${args.title}`,
          });

          exp.title = args.title;
          exp.amount_cents = args.amountCents;
          exp.paid_by = args.paidBy;
          exp.splits = args.splits;
          exp.updated_at = new Date().toISOString();
          return g;
        }

        case "delete_expense": {
          const g = groups.find((x) => x.id === args?.groupId);
          if (!g) throw new Error("Group not found");
          g.expenses = g.expenses.filter((x) => x.id !== args?.expenseId);
          return g;
        }

        case "record_reimbursement": {
          const g = groups.find((x) => x.id === args?.groupId);
          if (!g) throw new Error("Group not found");
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
          });
          return g;
        }

        case "get_balances": {
          const g = groups.find((x) => x.id === args?.groupId);
          if (!g) throw new Error("Group not found");
          return computeBalances(g);
        }

        case "get_settlements": {
          const g = groups.find((x) => x.id === args?.groupId);
          if (!g) throw new Error("Group not found");
          return computeSettlements(g);
        }

        default:
          throw new Error(`Unknown command: ${cmd}`);
      }
    },
  };
}
