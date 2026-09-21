import { invoke } from "@tauri-apps/api/core";
import type { ExpenseSplit, Group, ParticipantBalance, SettlementTransfer } from "../types";

const isTauri = () => typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

// --- BROWSER LOCALSTORAGE ENGINE (Fallback for Web Preview) ---
const STORAGE_KEY = "ezcount_mock_data_v1";

function getMockGroups(): Group[] {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) {
      const initial: Group[] = [
        {
          id: "group-demo-1",
          name: "Trip to Barcelona",
          currency: "EUR",
          created_at: new Date().toISOString(),
          participants: [
            { id: "p1", name: "Alice" },
            { id: "p2", name: "Bob" },
            { id: "p3", name: "Charlie" },
          ],
          expenses: [
            {
              id: "e1",
              group_id: "group-demo-1",
              title: "Airbnb Rental",
              amount_cents: 30000,
              paid_by: "p1",
              splits: [
                { participant_id: "p1", shares: 1 },
                { participant_id: "p2", shares: 1 },
                { participant_id: "p3", shares: 1 },
              ],
              created_at: new Date(Date.now() - 86400000 * 2).toISOString(),
              is_reimbursement: false,
            },
            {
              id: "e2",
              group_id: "group-demo-1",
              title: "Tapas & Sangria",
              amount_cents: 9000,
              paid_by: "p2",
              splits: [
                { participant_id: "p1", shares: 1 },
                { participant_id: "p2", shares: 1 },
                { participant_id: "p3", shares: 1 },
              ],
              created_at: new Date(Date.now() - 86400000).toISOString(),
              is_reimbursement: false,
            },
          ],
        },
      ];
      localStorage.setItem(STORAGE_KEY, JSON.stringify(initial));
      return initial;
    }
    return JSON.parse(raw);
  } catch {
    return [];
  }
}

function saveMockGroups(groups: Group[]) {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(groups));
}

function computeBalancesMock(group: Group): ParticipantBalance[] {
  const map = new Map<string, { paid: number; owed: number }>();
  for (const p of group.participants) {
    map.set(p.id, { paid: 0, owed: 0 });
  }

  for (const exp of group.expenses) {
    const payer = map.get(exp.paid_by);
    if (payer) {
      payer.paid += exp.amount_cents;
    }

    if (exp.splits && exp.splits.length > 0) {
      const totalShares = exp.splits.reduce((sum, s) => sum + s.shares, 0);
      if (totalShares > 0) {
        const allocated = exp.splits.map((s, idx) => {
          const base = Math.floor((exp.amount_cents * s.shares) / totalShares);
          const rem = (exp.amount_cents * s.shares) % totalShares;
          return { base, rem, idx, pid: s.participant_id };
        });

        const totalAllocated = allocated.reduce((sum, a) => sum + a.base, 0);
        const remainderCents = exp.amount_cents - totalAllocated;

        const order = [...allocated.keys()].sort((a, b) => allocated[b].rem - allocated[a].rem);
        for (let i = 0; i < remainderCents; i++) {
          allocated[order[i]].base += 1;
        }

        for (const item of allocated) {
          const debtor = map.get(item.pid);
          if (debtor) {
            debtor.owed += item.base;
          }
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

function computeSettlementsMock(group: Group): SettlementTransfer[] {
  const balances = computeBalancesMock(group);
  const debtors = balances
    .filter((b) => b.net_cents < 0)
    .map((b) => ({ ...b, net_cents: -b.net_cents }))
    .sort((a, b) => b.net_cents - a.net_cents);

  const creditors = balances
    .filter((b) => b.net_cents > 0)
    .map((b) => ({ ...b }))
    .sort((a, b) => b.net_cents - a.net_cents);

  const transfers: SettlementTransfer[] = [];
  let dIdx = 0;
  let cIdx = 0;

  while (dIdx < debtors.length && cIdx < creditors.length) {
    const debtor = debtors[dIdx];
    const creditor = creditors[cIdx];
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

    if (debtor.net_cents === 0) dIdx++;
    if (creditor.net_cents === 0) cIdx++;
  }

  return transfers;
}

// --- UNIFIED API BRIDGE ---
export const api = {
  async getGroups(): Promise<Group[]> {
    if (isTauri()) {
      return invoke<Group[]>("get_groups");
    }
    return getMockGroups();
  },

  async getGroup(id: string): Promise<Group> {
    if (isTauri()) {
      return invoke<Group>("get_group", { id });
    }
    const g = getMockGroups().find((x) => x.id === id);
    if (!g) throw new Error("Group not found");
    return g;
  },

  async createGroup(name: string, currency: string, participants: string[]): Promise<Group> {
    if (isTauri()) {
      return invoke<Group>("create_group", { name, currency, participants });
    }
    const groups = getMockGroups();
    const newGroup: Group = {
      id: `group-${Date.now()}`,
      name,
      currency,
      created_at: new Date().toISOString(),
      participants: participants.map((p, idx) => ({
        id: `p-${Date.now()}-${idx}`,
        name: p,
      })),
      expenses: [],
    };
    groups.unshift(newGroup);
    saveMockGroups(groups);
    return newGroup;
  },

  async deleteGroup(id: string): Promise<void> {
    if (isTauri()) {
      return invoke<void>("delete_group", { id });
    }
    const groups = getMockGroups().filter((g) => g.id !== id);
    saveMockGroups(groups);
  },

  async addParticipant(groupId: string, name: string): Promise<Group> {
    if (isTauri()) {
      return invoke<Group>("add_participant", { groupId, name });
    }
    const groups = getMockGroups();
    const group = groups.find((g) => g.id === groupId);
    if (!group) throw new Error("Group not found");
    group.participants.push({
      id: `p-${Date.now()}`,
      name,
    });
    saveMockGroups(groups);
    return group;
  },

  async addExpense(
    groupId: string,
    title: string,
    amountCents: number,
    paidBy: string,
    splits: ExpenseSplit[]
  ): Promise<Group> {
    if (isTauri()) {
      return invoke<Group>("add_expense", {
        groupId,
        title,
        amountCents,
        paidBy,
        splits,
      });
    }
    const groups = getMockGroups();
    const group = groups.find((g) => g.id === groupId);
    if (!group) throw new Error("Group not found");
    group.expenses.unshift({
      id: `e-${Date.now()}`,
      group_id: groupId,
      title,
      amount_cents: amountCents,
      paid_by: paidBy,
      splits,
      created_at: new Date().toISOString(),
      is_reimbursement: false,
    });
    saveMockGroups(groups);
    return group;
  },

  async deleteExpense(groupId: string, expenseId: string): Promise<Group> {
    if (isTauri()) {
      return invoke<Group>("delete_expense", { groupId, expenseId });
    }
    const groups = getMockGroups();
    const group = groups.find((g) => g.id === groupId);
    if (!group) throw new Error("Group not found");
    group.expenses = group.expenses.filter((e) => e.id !== expenseId);
    saveMockGroups(groups);
    return group;
  },

  async recordReimbursement(
    groupId: string,
    fromId: string,
    toId: string,
    amountCents: number,
    notes?: string
  ): Promise<Group> {
    if (isTauri()) {
      return invoke<Group>("record_reimbursement", {
        groupId,
        fromId,
        toId,
        amountCents,
        notes: notes || null,
      });
    }
    const groups = getMockGroups();
    const group = groups.find((g) => g.id === groupId);
    if (!group) throw new Error("Group not found");

    const fromP = group.participants.find((p) => p.id === fromId);
    const toP = group.participants.find((p) => p.id === toId);
    const fromName = fromP ? fromP.name : "Unknown";
    const toName = toP ? toP.name : "Unknown";

    const title = notes?.trim()
      ? `Payment: ${fromName} → ${toName} (${notes.trim()})`
      : `Payment: ${fromName} → ${toName}`;

    group.expenses.unshift({
      id: `e-${Date.now()}`,
      group_id: groupId,
      title,
      amount_cents: amountCents,
      paid_by: fromId,
      splits: [{ participant_id: toId, shares: 1 }],
      created_at: new Date().toISOString(),
      is_reimbursement: true,
    });
    saveMockGroups(groups);
    return group;
  },

  async getBalances(groupId: string): Promise<ParticipantBalance[]> {
    if (isTauri()) {
      return invoke<ParticipantBalance[]>("get_balances", { groupId });
    }
    const group = getMockGroups().find((g) => g.id === groupId);
    if (!group) return [];
    return computeBalancesMock(group);
  },

  async getSettlements(groupId: string): Promise<SettlementTransfer[]> {
    if (isTauri()) {
      return invoke<SettlementTransfer[]>("get_settlements", { groupId });
    }
    const group = getMockGroups().find((g) => g.id === groupId);
    if (!group) return [];
    return computeSettlementsMock(group);
  },
};
