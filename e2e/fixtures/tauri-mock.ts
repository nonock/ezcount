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
  participants: { id: string; name: string; removed?: boolean }[];
  expenses: MockExpense[];
  created_at: string;
}

export function installTauriMock() {
  let groups: MockGroup[] | null = null;
  function getGroups(): MockGroup[] {
    if (!groups) {
      groups = (window as any).__SEED_GROUPS__ ? JSON.parse(JSON.stringify((window as any).__SEED_GROUPS__)) : [];
    }
    return groups;
  }

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

  function clone<T>(val: T): T {
    return JSON.parse(JSON.stringify(val));
  }

  // Sync state per group, and groups "on the relay" that can be joined (seeded by tests).
  const syncInfos = new Map<string, any>();
  function syncInfo(groupId: string) {
    return (
      syncInfos.get(groupId) || {
        group_id: groupId,
        enabled: false,
        server_url: null,
        invite_code: null,
        last_synced_at: null,
        last_error: null,
      }
    );
  }
  function inviteFor(serverUrl: string, groupId: string) {
    return `ezcount://join?server=${encodeURIComponent(serverUrl)}&group=${groupId}&key=mock-key&v=2`;
  }

  // Minimal event plugin: `listen` registers a callback that tests can fire with
  // `window.__emitMockEvent(name, payload)`.
  let nextCallbackId = 1;
  const callbacks = new Map<number, (event: any) => void>();
  const listeners = new Map<string, number[]>();
  (window as any).__TAURI_EVENT_PLUGIN_INTERNALS__ = {
    unregisterListener: (event: string, id: number) => {
      listeners.set(
        event,
        (listeners.get(event) || []).filter((x) => x !== id)
      );
    },
  };
  (window as any).__emitMockEvent = (event: string, payload: unknown) => {
    for (const id of listeners.get(event) || []) {
      callbacks.get(id)?.({ event, id, payload });
    }
  };

  (window as any).__TAURI_INTERNALS__ = {
    transformCallback: (callback: (event: any) => void) => {
      const id = nextCallbackId++;
      callbacks.set(id, callback);
      return id;
    },
    invoke: async (cmd: string, args?: any) => {
      switch (cmd) {
        case "plugin:event|listen": {
          listeners.set(args.event, [...(listeners.get(args.event) || []), args.handler]);
          return args.handler;
        }

        case "plugin:event|unlisten":
          return null;

        case "get_storage_warnings":
          return clone((window as any).__STORAGE_WARNINGS__ || []);

        case "get_sync_info": {
          if (!getGroups().some((x) => x.id === args?.groupId)) throw new Error("Group not found");
          return clone(syncInfo(args.groupId));
        }

        case "enable_sync": {
          if (!/^https?:\/\//.test(args?.serverUrl || "")) {
            throw new Error("The server URL must start with http:// or https://");
          }
          const info = {
            group_id: args.groupId,
            enabled: true,
            server_url: args.serverUrl.replace(/\/+$/, ""),
            invite_code: inviteFor(args.serverUrl.replace(/\/+$/, ""), args.groupId),
            last_synced_at: new Date().toISOString(),
            last_error: null,
          };
          syncInfos.set(args.groupId, info);
          return clone(info);
        }

        case "sync_now": {
          const info = { ...syncInfo(args.groupId), last_synced_at: new Date().toISOString() };
          syncInfos.set(args.groupId, info);
          return clone(info);
        }

        case "join_group": {
          const code = String(args?.inviteCode || "");
          if (!code.startsWith("ezcount://join?")) {
            throw new Error("This is not a valid ezcount invite code");
          }
          const params = new URL(code.replace("ezcount://", "https://")).searchParams;
          const groupId = params.get("group") || "";
          if (getGroups().some((x) => x.id === groupId)) {
            throw new Error("This group is already on this device");
          }
          const remote = ((window as any).__REMOTE_GROUPS__ || []).find(
            (g: MockGroup) => g.id === groupId
          );
          if (!remote) throw new Error("The sync server does not know this group");
          getGroups().push(clone(remote));
          const server = params.get("server") || "";
          syncInfos.set(groupId, {
            group_id: groupId,
            enabled: true,
            server_url: server,
            invite_code: inviteFor(server, groupId),
            last_synced_at: new Date().toISOString(),
            last_error: null,
          });
          return clone(remote);
        }

        case "remove_participant": {
          const g = getGroups().find((x) => x.id === args?.groupId);
          if (!g) throw new Error("Group not found");
          const p = g.participants.find((x) => x.id === args?.participantId);
          if (!p) throw new Error("Participant not found");
          p.removed = true;
          return clone(g);
        }

        case "get_groups":
          return clone(getGroups());

        case "get_group": {
          if (!args || typeof args.groupId !== "string") {
            throw new Error("missing required argument `group_id`");
          }
          const g = getGroups().find((x) => x.id === args.groupId);
          if (!g) throw new Error("Group not found");
          return clone(g);
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
          getGroups().unshift(newGroup);
          return clone(newGroup);
        }

        case "delete_group": {
          if (!args || typeof args.groupId !== "string") {
            throw new Error("missing required argument `group_id`");
          }
          const idx = getGroups().findIndex((x) => x.id === args.groupId);
          if (idx !== -1) getGroups().splice(idx, 1);
          return true;
        }

        case "add_participant": {
          const g = getGroups().find((x) => x.id === args?.groupId);
          if (!g) throw new Error("Group not found");
          g.participants.push({ id: `p-${Date.now()}`, name: args.name });
          return clone(g);
        }

        case "add_expense": {
          const g = getGroups().find((x) => x.id === args?.groupId);
          if (!g) throw new Error("Group not found");
          const now = new Date().toISOString();
          const createdAt = args?.createdAt || now;
          const exp: MockExpense = {
            id: `exp-${Date.now()}`,
            group_id: args.groupId,
            title: args.title,
            amount_cents: args.amountCents,
            paid_by: args.paidBy,
            splits: args.splits,
            created_at: createdAt,
            updated_at: now,
            history: [],
            is_reimbursement: false,
          };
          g.expenses.unshift(exp);
          return clone(g);
        }

        case "update_expense": {
          const g = getGroups().find((x) => x.id === args?.groupId);
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
          if (args?.createdAt) {
            exp.created_at = args.createdAt;
          }
          exp.updated_at = new Date().toISOString();
          return clone(g);
        }

        case "delete_expense": {
          const g = getGroups().find((x) => x.id === args?.groupId);
          if (!g) throw new Error("Group not found");
          g.expenses = g.expenses.filter((x) => x.id !== args?.expenseId);
          return clone(g);
        }

        case "record_reimbursement": {
          const g = getGroups().find((x) => x.id === args?.groupId);
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
          return clone(g);
        }

        case "get_balances": {
          const g = getGroups().find((x) => x.id === args?.groupId);
          if (!g) throw new Error("Group not found");
          return clone(computeBalances(g));
        }

        case "get_settlements": {
          const g = getGroups().find((x) => x.id === args?.groupId);
          if (!g) throw new Error("Group not found");
          return clone(computeSettlements(g));
        }

        default:
          throw new Error(`Unknown command: ${cmd}`);
      }
    },
  };
}
