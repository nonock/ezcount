// ezcount — Tricount-like Expense Sharing Application
import { invoke } from "@tauri-apps/api/core";

// ==========================================
// 1. DATA MODELS & TYPES
// ==========================================
export interface Participant {
  id: string;
  name: string;
}

export interface Expense {
  id: string;
  group_id: string;
  title: string;
  amount_cents: number;
  paid_by: string;
  split_among: string[];
  created_at: string;
  is_reimbursement?: boolean;
}

export interface Group {
  id: string;
  name: string;
  currency: string;
  participants: Participant[];
  expenses: Expense[];
  created_at: string;
}

export interface ParticipantBalance {
  participant_id: string;
  participant_name: string;
  paid_cents: number;
  owed_cents: number;
  net_cents: number;
}

export interface SettlementTransfer {
  from_id: string;
  from_name: string;
  to_id: string;
  to_name: string;
  amount_cents: number;
}

// ==========================================
// 2. DUAL-RUNTIME IPC CLIENT (Tauri & Browser Fallback)
// ==========================================
const isTauri =
  typeof window !== "undefined" && ("__TAURI_INTERNALS__" in window || "__TAURI__" in window);

// In-browser mock store for smooth local web testing (`bun run dev`)
const MOCK_STORAGE_KEY = "ezcount_mock_data";

function getLocalMockGroups(): Group[] {
  try {
    const raw = localStorage.getItem(MOCK_STORAGE_KEY);
    if (!raw) {
      // Seed with initial example group
      const initial: Group[] = [
        {
          id: "group_demo",
          name: "Alps Ski Weekend",
          currency: "EUR",
          created_at: new Date(Date.now() - 86400000 * 2).toISOString(),
          participants: [
            { id: "p1", name: "Alice" },
            { id: "p2", name: "Bob" },
            { id: "p3", name: "Charlie" },
          ],
          expenses: [
            {
              id: "e1",
              group_id: "group_demo",
              title: "Chalet Rental",
              amount_cents: 30000,
              paid_by: "p1",
              split_among: ["p1", "p2", "p3"],
              created_at: new Date(Date.now() - 86400000).toISOString(),
              is_reimbursement: false,
            },
            {
              id: "e2",
              group_id: "group_demo",
              title: "Groceries & Fondue",
              amount_cents: 7500,
              paid_by: "p2",
              split_among: ["p1", "p2", "p3"],
              created_at: new Date().toISOString(),
              is_reimbursement: false,
            },
          ],
        },
      ];
      localStorage.setItem(MOCK_STORAGE_KEY, JSON.stringify(initial));
      return initial;
    }
    return JSON.parse(raw);
  } catch {
    return [];
  }
}

function saveLocalMockGroups(groups: Group[]) {
  localStorage.setItem(MOCK_STORAGE_KEY, JSON.stringify(groups));
}

function mockCalculateBalances(group: Group): ParticipantBalance[] {
  const paidMap = new Map<string, number>();
  const owedMap = new Map<string, number>();

  for (const p of group.participants) {
    paidMap.set(p.id, 0);
    owedMap.set(p.id, 0);
  }

  for (const expense of group.expenses) {
    paidMap.set(expense.paid_by, (paidMap.get(expense.paid_by) || 0) + expense.amount_cents);

    const count = expense.split_among.length;
    if (count > 0) {
      const baseShare = Math.floor(expense.amount_cents / count);
      const remainder = expense.amount_cents % count;

      for (let idx = 0; idx < expense.split_among.length; idx++) {
        const memberId = expense.split_among[idx];
        const share = idx < remainder ? baseShare + 1 : baseShare;
        owedMap.set(memberId, (owedMap.get(memberId) || 0) + share);
      }
    }
  }

  return group.participants.map((p) => {
    const paid = paidMap.get(p.id) || 0;
    const owed = owedMap.get(p.id) || 0;
    return {
      participant_id: p.id,
      participant_name: p.name,
      paid_cents: paid,
      owed_cents: owed,
      net_cents: paid - owed,
    };
  });
}

function mockCalculateSettlements(group: Group): SettlementTransfer[] {
  const balances = mockCalculateBalances(group);
  const nameMap = new Map(group.participants.map((p) => [p.id, p.name]));

  const debtors = balances
    .filter((b) => b.net_cents < 0)
    .map((b) => ({ id: b.participant_id, owed: -b.net_cents }))
    .sort((a, b) => b.owed - a.owed);

  const creditors = balances
    .filter((b) => b.net_cents > 0)
    .map((b) => ({ id: b.participant_id, owed: b.net_cents }))
    .sort((a, b) => b.owed - a.owed);

  const transfers: SettlementTransfer[] = [];
  let d = 0;
  let c = 0;

  while (d < debtors.length && c < creditors.length) {
    const settle = Math.min(debtors[d].owed, creditors[c].owed);
    if (settle > 0) {
      transfers.push({
        from_id: debtors[d].id,
        from_name: nameMap.get(debtors[d].id) || "Unknown",
        to_id: creditors[c].id,
        to_name: nameMap.get(creditors[c].id) || "Unknown",
        amount_cents: settle,
      });
    }

    debtors[d].owed -= settle;
    creditors[c].owed -= settle;

    if (debtors[d].owed === 0) d++;
    if (creditors[c].owed === 0) c++;
  }

  return transfers;
}

// Unified API bridge
const api = {
  async getGroups(): Promise<Group[]> {
    if (isTauri) {
      return await invoke("get_groups");
    }
    return getLocalMockGroups();
  },

  async getGroup(groupId: string): Promise<Group | null> {
    if (isTauri) {
      return await invoke("get_group", { groupId });
    }
    const groups = getLocalMockGroups();
    return groups.find((g) => g.id === groupId) || null;
  },

  async createGroup(name: string, currency: string, participants: string[]): Promise<Group> {
    if (isTauri) {
      return await invoke("create_group", { name, currency, participants });
    }
    const groups = getLocalMockGroups();
    const newGroup: Group = {
      id: `grp_${Math.random().toString(36).substring(2, 9)}`,
      name: name.trim(),
      currency: currency.toUpperCase(),
      created_at: new Date().toISOString(),
      participants: participants.map((n) => ({
        id: `p_${Math.random().toString(36).substring(2, 9)}`,
        name: n.trim(),
      })),
      expenses: [],
    };
    groups.push(newGroup);
    saveLocalMockGroups(groups);
    return newGroup;
  },

  async deleteGroup(groupId: string): Promise<boolean> {
    if (isTauri) {
      return await invoke("delete_group", { groupId });
    }
    const groups = getLocalMockGroups().filter((g) => g.id !== groupId);
    saveLocalMockGroups(groups);
    return true;
  },

  async addParticipant(groupId: string, name: string): Promise<Group> {
    if (isTauri) {
      return await invoke("add_participant", { groupId, name });
    }
    const groups = getLocalMockGroups();
    const group = groups.find((g) => g.id === groupId);
    if (!group) throw new Error("Group not found");
    group.participants.push({
      id: `p_${Math.random().toString(36).substring(2, 9)}`,
      name: name.trim(),
    });
    saveLocalMockGroups(groups);
    return group;
  },

  async addExpense(
    groupId: string,
    title: string,
    amountCents: number,
    paidBy: string,
    splitAmong: string[]
  ): Promise<Group> {
    if (isTauri) {
      return await invoke("add_expense", {
        groupId,
        title,
        amountCents,
        paidBy,
        splitAmong,
      });
    }
    const groups = getLocalMockGroups();
    const group = groups.find((g) => g.id === groupId);
    if (!group) throw new Error("Group not found");
    group.expenses.push({
      id: `exp_${Math.random().toString(36).substring(2, 9)}`,
      group_id: groupId,
      title: title.trim(),
      amount_cents: amountCents,
      paid_by: paidBy,
      split_among: splitAmong,
      created_at: new Date().toISOString(),
      is_reimbursement: false,
    });
    saveLocalMockGroups(groups);
    return group;
  },

  async recordReimbursement(
    groupId: string,
    fromId: string,
    toId: string,
    amountCents: number,
    notes?: string
  ): Promise<Group> {
    if (isTauri) {
      return await invoke("record_reimbursement", {
        groupId,
        fromId,
        toId,
        amountCents,
        notes: notes || null,
      });
    }
    const groups = getLocalMockGroups();
    const group = groups.find((g) => g.id === groupId);
    if (!group) throw new Error("Group not found");
    const fromMember = group.participants.find((p) => p.id === fromId);
    const toMember = group.participants.find((p) => p.id === toId);
    const fromName = fromMember ? fromMember.name : "Unknown";
    const toName = toMember ? toMember.name : "Unknown";
    const title = notes?.trim()
      ? `Payment: ${fromName} → ${toName} (${notes.trim()})`
      : `Payment: ${fromName} → ${toName}`;

    group.expenses.push({
      id: `exp_${Math.random().toString(36).substring(2, 9)}`,
      group_id: groupId,
      title,
      amount_cents: amountCents,
      paid_by: fromId,
      split_among: [toId],
      created_at: new Date().toISOString(),
      is_reimbursement: true,
    });
    saveLocalMockGroups(groups);
    return group;
  },

  async deleteExpense(groupId: string, expenseId: string): Promise<Group> {
    if (isTauri) {
      return await invoke("delete_expense", { groupId, expenseId });
    }
    const groups = getLocalMockGroups();
    const group = groups.find((g) => g.id === groupId);
    if (!group) throw new Error("Group not found");
    group.expenses = group.expenses.filter((e) => e.id !== expenseId);
    saveLocalMockGroups(groups);
    return group;
  },

  async getBalances(groupId: string): Promise<ParticipantBalance[]> {
    if (isTauri) {
      return await invoke("get_balances", { groupId });
    }
    const group = await api.getGroup(groupId);
    return group ? mockCalculateBalances(group) : [];
  },

  async getSettlements(groupId: string): Promise<SettlementTransfer[]> {
    if (isTauri) {
      return await invoke("get_settlements", { groupId });
    }
    const group = await api.getGroup(groupId);
    return group ? mockCalculateSettlements(group) : [];
  },
};

// ==========================================
// 3. UI STATE & HELPERS
// ==========================================
let currentGroup: Group | null = null;
let currentTab: "expenses" | "balances" | "settle" = "expenses";

const CURRENCY_SYMBOLS: Record<string, string> = {
  EUR: "€",
  USD: "$",
  GBP: "£",
  CHF: "CHF",
  CAD: "C$",
};

function formatMoney(amountCents: number, currency = "EUR"): string {
  const symbol = CURRENCY_SYMBOLS[currency.toUpperCase()] || currency;
  const isNegative = amountCents < 0;
  const absCents = Math.abs(amountCents);
  const formatted = (absCents / 100).toFixed(2);
  return `${isNegative ? "-" : ""}${formatted} ${symbol}`;
}

function formatDate(iso: string): string {
  try {
    const d = new Date(iso);
    return d.toLocaleDateString(undefined, {
      month: "short",
      day: "numeric",
      year: "numeric",
    });
  } catch {
    return iso;
  }
}

// ==========================================
// 4. RENDERING FUNCTIONS
// ==========================================

async function loadDashboard() {
  currentGroup = null;
  document.getElementById("view-dashboard")?.classList.remove("hidden");
  document.getElementById("view-group")?.classList.add("hidden");
  document.getElementById("nav-active-group")?.classList.add("hidden");

  const groups = await api.getGroups();
  const countBadge = document.getElementById("groups-count-badge");
  if (countBadge) countBadge.textContent = groups.length.toString();

  const grid = document.getElementById("groups-grid");
  const emptyState = document.getElementById("groups-empty-state");

  if (!grid || !emptyState) return;

  if (groups.length === 0) {
    grid.innerHTML = "";
    emptyState.classList.remove("hidden");
    return;
  }

  emptyState.classList.add("hidden");
  grid.innerHTML = groups
    .map((g) => {
      const totalCents = g.expenses
        .filter((e) => !e.is_reimbursement)
        .reduce((sum, e) => sum + e.amount_cents, 0);
      const participantPills = g.participants
        .slice(0, 4)
        .map(
          (p) =>
            `<span class="px-2 py-0.5 rounded-full bg-slate-800 text-[11px] text-slate-300 font-medium">${p.name}</span>`
        )
        .join("");
      const moreCount = g.participants.length > 4 ? `+${g.participants.length - 4}` : "";

      return `
        <div data-group-id="${g.id}" class="group-card group relative p-5 rounded-2xl border border-slate-800 bg-slate-900/60 hover:bg-slate-900 hover:border-slate-700 transition duration-200 cursor-pointer shadow-lg hover:shadow-indigo-500/5">
          <div class="flex items-start justify-between gap-2 mb-3">
            <div>
              <h3 class="text-base font-bold text-white group-hover:text-indigo-400 transition">${g.name}</h3>
              <p class="text-xs text-slate-400 mt-0.5">Created ${formatDate(g.created_at)}</p>
            </div>
            <span class="px-2 py-0.5 rounded text-xs font-semibold bg-indigo-500/10 text-indigo-400 border border-indigo-500/20">${g.currency}</span>
          </div>

          <div class="flex items-baseline justify-between border-t border-slate-800/80 pt-3 mt-4">
            <div>
              <span class="text-[11px] text-slate-500 block">Group Spending</span>
              <span class="text-sm font-bold text-slate-200">${formatMoney(totalCents, g.currency)}</span>
            </div>
            <div class="text-right">
              <span class="text-[11px] text-slate-500 block">Transactions</span>
              <span class="text-xs font-semibold text-slate-300">${g.expenses.length}</span>
            </div>
          </div>

          <div class="flex items-center gap-1.5 mt-3 pt-2">
            ${participantPills}
            ${moreCount ? `<span class="text-[11px] text-slate-500 font-medium">${moreCount}</span>` : ""}
          </div>
        </div>
      `;
    })
    .join("");

  // Attach click listeners to group cards
  for (const card of grid.querySelectorAll(".group-card")) {
    card.addEventListener("click", () => {
      const gid = card.getAttribute("data-group-id");
      if (gid) openGroup(gid);
    });
  }
}

async function openGroup(groupId: string) {
  const group = await api.getGroup(groupId);
  if (!group) return;
  currentGroup = group;

  document.getElementById("view-dashboard")?.classList.add("hidden");
  document.getElementById("view-group")?.classList.remove("hidden");

  // Navbar indicator
  const navActive = document.getElementById("nav-active-group");
  const navName = document.getElementById("nav-active-group-name");
  const navCurr = document.getElementById("nav-active-group-currency");
  if (navActive && navName && navCurr) {
    navName.textContent = group.name;
    navCurr.textContent = group.currency;
    navActive.classList.remove("hidden");
    navActive.classList.add("flex");
  }

  // Header banner info
  const titleEl = document.getElementById("group-title");
  const currEl = document.getElementById("group-currency-badge");
  const dateEl = document.getElementById("group-created-date");
  if (titleEl) titleEl.textContent = group.name;
  if (currEl) currEl.textContent = group.currency;
  if (dateEl) dateEl.textContent = `Created on ${formatDate(group.created_at)}`;

  // Quick stats: Spending excludes direct reimbursements to represent true group costs
  const totalCents = group.expenses
    .filter((e) => !e.is_reimbursement)
    .reduce((sum, e) => sum + e.amount_cents, 0);
  const totalSpentEl = document.getElementById("stat-total-spent");
  const expCountEl = document.getElementById("stat-expense-count");
  const partCountEl = document.getElementById("stat-participant-count");
  if (totalSpentEl) totalSpentEl.textContent = formatMoney(totalCents, group.currency);
  if (expCountEl) expCountEl.textContent = group.expenses.length.toString();
  if (partCountEl) partCountEl.textContent = group.participants.length.toString();

  // Participant list chips
  const partListEl = document.getElementById("group-participants-list");
  if (partListEl) {
    partListEl.innerHTML = group.participants
      .map(
        (p) =>
          `<span class="px-2.5 py-1 rounded-full bg-slate-800 border border-slate-700/60 text-xs font-medium text-slate-200">${p.name}</span>`
      )
      .join("");
  }

  // Render current tab
  switchTab(currentTab);
}

async function renderExpensesTab(group: Group) {
  const container = document.getElementById("expenses-list");
  const empty = document.getElementById("expenses-empty-state");
  if (!container || !empty) return;

  if (group.expenses.length === 0) {
    container.innerHTML = "";
    empty.classList.remove("hidden");
    return;
  }

  empty.classList.add("hidden");
  const nameMap = new Map(group.participants.map((p) => [p.id, p.name]));

  // Sort expenses by most recent first
  const sortedExpenses = [...group.expenses].sort(
    (a, b) => new Date(b.created_at).getTime() - new Date(a.created_at).getTime()
  );

  container.innerHTML = sortedExpenses
    .map((e) => {
      const payerName = nameMap.get(e.paid_by) || "Unknown";
      const isReimbursement = !!e.is_reimbursement;

      if (isReimbursement) {
        const recipientName = nameMap.get(e.split_among[0]) || "Unknown";
        return `
          <div class="flex items-center justify-between p-4 rounded-xl border border-emerald-500/20 bg-emerald-950/10 hover:bg-emerald-950/20 transition group">
            <div class="flex items-center gap-3.5">
              <div class="w-10 h-10 rounded-xl bg-emerald-500/10 border border-emerald-500/30 flex items-center justify-center text-emerald-400 font-bold text-base">
                🤝
              </div>
              <div>
                <div class="flex items-center gap-2">
                  <h4 class="text-sm font-semibold text-white">${e.title}</h4>
                  <span class="px-2 py-0.5 rounded text-[10px] font-bold bg-emerald-500/20 text-emerald-300 border border-emerald-500/30">Reimbursement</span>
                </div>
                <p class="text-xs text-slate-400 mt-0.5">
                  <span class="font-medium text-slate-200">${payerName}</span> paid <span class="font-medium text-slate-200">${recipientName}</span> directly
                </p>
                <p class="text-[11px] text-slate-500 mt-0.5">${formatDate(e.created_at)}</p>
              </div>
            </div>

            <div class="flex items-center gap-3">
              <div class="text-right">
                <span class="text-sm font-bold text-emerald-400">${formatMoney(e.amount_cents, group.currency)}</span>
              </div>
              <button data-expense-id="${e.id}" class="btn-delete-expense opacity-40 group-hover:opacity-100 p-1.5 rounded-lg text-slate-400 hover:text-rose-400 hover:bg-rose-500/10 transition cursor-pointer" title="Delete reimbursement">
                <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                  <path stroke-linecap="round" stroke-linejoin="round" d="m14.74 9-.346 9m-4.788 0L9.26 9m9.968-3.21c.342.052.682.107 1.022.166m-1.022-.165L18.16 19.673a2.25 2.25 0 0 1-2.244 2.077H8.084a2.25 2.25 0 0 1-2.244-2.077L4.772 5.79m14.456 0a48.108 48.108 0 0 0-3.478-.397m-12 .562c.34-.059.68-.114 1.022-.165m0 0a48.11 48.11 0 0 1 3.478-.397m7.5 0v-.916c0-1.18-.91-2.164-2.09-2.201a51.964 51.964 0 0 0-3.32 0c-1.18.037-2.09 1.022-2.09 2.201v.916m7.5 0a48.667 48.667 0 0 0-7.5 0" />
                </svg>
              </button>
            </div>
          </div>
        `;
      }

      const splitNames = e.split_among.map((id) => nameMap.get(id) || "Unknown").join(", ");
      const sharePerPerson = formatMoney(
        Math.floor(e.amount_cents / (e.split_among.length || 1)),
        group.currency
      );

      return `
        <div class="flex items-center justify-between p-4 rounded-xl border border-slate-800 bg-slate-900/50 hover:bg-slate-900/90 transition group">
          <div class="flex items-center gap-3.5">
            <div class="w-10 h-10 rounded-xl bg-slate-800/80 border border-slate-700/50 flex items-center justify-center text-indigo-400 font-bold text-sm">
              ${e.title.charAt(0).toUpperCase()}
            </div>
            <div>
              <h4 class="text-sm font-semibold text-white">${e.title}</h4>
              <p class="text-xs text-slate-400 mt-0.5">
                Paid by <span class="font-medium text-slate-300">${payerName}</span> • for ${e.split_among.length} members (${sharePerPerson} each)
              </p>
              <p class="text-[11px] text-slate-500 mt-0.5">${formatDate(e.created_at)} • [${splitNames}]</p>
            </div>
          </div>

          <div class="flex items-center gap-3">
            <div class="text-right">
              <span class="text-sm font-bold text-white">${formatMoney(e.amount_cents, group.currency)}</span>
            </div>
            <button data-expense-id="${e.id}" class="btn-delete-expense opacity-40 group-hover:opacity-100 p-1.5 rounded-lg text-slate-400 hover:text-rose-400 hover:bg-rose-500/10 transition cursor-pointer" title="Delete expense">
              <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                <path stroke-linecap="round" stroke-linejoin="round" d="m14.74 9-.346 9m-4.788 0L9.26 9m9.968-3.21c.342.052.682.107 1.022.166m-1.022-.165L18.16 19.673a2.25 2.25 0 0 1-2.244 2.077H8.084a2.25 2.25 0 0 1-2.244-2.077L4.772 5.79m14.456 0a48.108 48.108 0 0 0-3.478-.397m-12 .562c.34-.059.68-.114 1.022-.165m0 0a48.11 48.11 0 0 1 3.478-.397m7.5 0v-.916c0-1.18-.91-2.164-2.09-2.201a51.964 51.964 0 0 0-3.32 0c-1.18.037-2.09 1.022-2.09 2.201v.916m7.5 0a48.667 48.667 0 0 0-7.5 0" />
              </svg>
            </button>
          </div>
        </div>
      `;
    })
    .join("");

  for (const btn of container.querySelectorAll(".btn-delete-expense")) {
    btn.addEventListener("click", async (e) => {
      e.stopPropagation();
      const eid = btn.getAttribute("data-expense-id");
      if (eid && currentGroup && confirm("Are you sure you want to delete this record?")) {
        const updated = await api.deleteExpense(currentGroup.id, eid);
        currentGroup = updated;
        openGroup(updated.id);
      }
    });
  }
}

async function renderBalancesTab(group: Group) {
  const container = document.getElementById("balances-list");
  if (!container) return;

  const balances = await api.getBalances(group.id);
  const maxAbs = Math.max(...balances.map((b) => Math.abs(b.net_cents)), 1);

  container.innerHTML = balances
    .map((b) => {
      const isPositive = b.net_cents > 0;
      const isNegative = b.net_cents < 0;
      const isEven = b.net_cents === 0;

      const percentage = Math.min(100, Math.round((Math.abs(b.net_cents) / maxAbs) * 100));

      const badgeColor = isPositive
        ? "text-emerald-400 bg-emerald-500/10 border-emerald-500/20"
        : isNegative
          ? "text-rose-400 bg-rose-500/10 border-rose-500/20"
          : "text-slate-400 bg-slate-800 border-slate-700";

      const barColor = isPositive ? "bg-emerald-500" : isNegative ? "bg-rose-500" : "bg-slate-700";

      const statusText = isPositive ? "Gets back" : isNegative ? "Owes" : "Settled up";

      return `
        <div class="p-4 rounded-xl border border-slate-800 bg-slate-900/50 space-y-3">
          <div class="flex items-center justify-between">
            <div class="flex items-center gap-2.5">
              <div class="w-8 h-8 rounded-full bg-slate-800 flex items-center justify-center text-xs font-bold text-slate-200">
                ${b.participant_name.charAt(0).toUpperCase()}
              </div>
              <div>
                <h4 class="text-sm font-semibold text-white">${b.participant_name}</h4>
                <span class="text-[11px] text-slate-400">${statusText}</span>
              </div>
            </div>
            <div class="px-2.5 py-1 rounded-lg border text-xs font-bold ${badgeColor}">
              ${formatMoney(b.net_cents, group.currency)}
            </div>
          </div>

          <!-- Progress bar -->
          <div class="w-full bg-slate-950 rounded-full h-2 overflow-hidden">
            <div class="${barColor} h-2 rounded-full transition-all duration-300" style="width: ${isEven ? 0 : percentage}%"></div>
          </div>

          <div class="flex items-center justify-between text-[11px] text-slate-400 pt-1 border-t border-slate-800/60">
            <span>Paid: <strong class="text-slate-200">${formatMoney(b.paid_cents, group.currency)}</strong></span>
            <span>Consumed: <strong class="text-slate-200">${formatMoney(b.owed_cents, group.currency)}</strong></span>
          </div>

          ${
            isNegative
              ? `
            <div class="pt-1">
              <button data-participant-id="${b.participant_id}" data-amount="${(Math.abs(b.net_cents) / 100).toFixed(2)}" class="btn-balance-reimburse w-full py-1.5 rounded-lg text-xs font-semibold bg-slate-800/80 hover:bg-emerald-950/50 text-slate-300 hover:text-emerald-300 border border-slate-700 hover:border-emerald-500/40 transition flex items-center justify-center gap-1.5 cursor-pointer">
                <span>Reimburse debt</span>
                <span class="text-[10px] text-slate-400 font-mono font-normal">(${formatMoney(Math.abs(b.net_cents), group.currency)})</span>
              </button>
            </div>
          `
              : ""
          }
        </div>
      `;
    })
    .join("");

  for (const btn of container.querySelectorAll(".btn-balance-reimburse")) {
    btn.addEventListener("click", () => {
      const pid = btn.getAttribute("data-participant-id") || undefined;
      const amount = btn.getAttribute("data-amount") || undefined;
      openReimburseModal(pid, undefined, amount);
    });
  }
}

async function renderSettlementsTab(group: Group) {
  const container = document.getElementById("settlements-list");
  const empty = document.getElementById("settlements-empty-state");
  if (!container || !empty) return;

  const settlements = await api.getSettlements(group.id);

  if (settlements.length === 0) {
    container.innerHTML = "";
    empty.classList.remove("hidden");
    return;
  }

  empty.classList.add("hidden");
  container.innerHTML = settlements
    .map(
      (s) => `
      <div class="flex flex-col sm:flex-row sm:items-center justify-between gap-3 p-4 rounded-xl border border-slate-800 bg-slate-900/50 hover:bg-slate-900/80 transition">
        <div class="flex items-center gap-3">
          <div class="w-8 h-8 rounded-full bg-rose-500/10 text-rose-400 border border-rose-500/20 flex items-center justify-center text-xs font-bold">
            ${s.from_name.charAt(0).toUpperCase()}
          </div>
          <span class="text-sm font-medium text-slate-300">
            <strong class="text-white">${s.from_name}</strong>
            <span class="text-slate-400 text-xs px-2">pays</span>
            <strong class="text-white">${s.to_name}</strong>
          </span>
          <div class="w-8 h-8 rounded-full bg-emerald-500/10 text-emerald-400 border border-emerald-500/20 flex items-center justify-center text-xs font-bold">
            ${s.to_name.charAt(0).toUpperCase()}
          </div>
        </div>

        <div class="flex items-center justify-between sm:justify-end gap-3 pt-2 sm:pt-0 border-t sm:border-t-0 border-slate-800/80">
          <span class="text-base font-bold text-emerald-400 font-mono">
            ${formatMoney(s.amount_cents, group.currency)}
          </span>
          <button data-from-id="${s.from_id}" data-to-id="${s.to_id}" data-amount="${(s.amount_cents / 100).toFixed(2)}" class="btn-mark-paid inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-semibold bg-emerald-600/20 hover:bg-emerald-600 text-emerald-300 hover:text-white border border-emerald-500/30 transition cursor-pointer active:scale-95">
            <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.5"><path stroke-linecap="round" stroke-linejoin="round" d="m4.5 12.75 6 6 9-13.5" /></svg>
            <span>Mark as Paid</span>
          </button>
        </div>
      </div>
    `
    )
    .join("");

  // Attach click listeners to Mark as Paid buttons to open pre-filled reimbursement modal
  for (const btn of container.querySelectorAll(".btn-mark-paid")) {
    btn.addEventListener("click", () => {
      const fromId = btn.getAttribute("data-from-id") || undefined;
      const toId = btn.getAttribute("data-to-id") || undefined;
      const amount = btn.getAttribute("data-amount") || undefined;
      openReimburseModal(fromId, toId, amount);
    });
  }
}

function switchTab(tab: "expenses" | "balances" | "settle") {
  currentTab = tab;

  // Toggle active tab buttons
  const buttons: Record<string, HTMLElement | null> = {
    expenses: document.getElementById("tab-btn-expenses"),
    balances: document.getElementById("tab-btn-balances"),
    settle: document.getElementById("tab-btn-settle"),
  };

  const panels: Record<string, HTMLElement | null> = {
    expenses: document.getElementById("tab-panel-expenses"),
    balances: document.getElementById("tab-panel-balances"),
    settle: document.getElementById("tab-panel-settle"),
  };

  for (const [key, btn] of Object.entries(buttons)) {
    if (btn) {
      if (key === tab) {
        btn.classList.add("border-indigo-500", "text-indigo-400");
        btn.classList.remove("border-transparent", "text-slate-400");
      } else {
        btn.classList.remove("border-indigo-500", "text-indigo-400");
        btn.classList.add("border-transparent", "text-slate-400");
      }
    }
  }

  for (const [key, panel] of Object.entries(panels)) {
    if (panel) {
      if (key === tab) panel.classList.remove("hidden");
      else panel.classList.add("hidden");
    }
  }

  // Load content
  if (currentGroup) {
    if (tab === "expenses") renderExpensesTab(currentGroup);
    else if (tab === "balances") renderBalancesTab(currentGroup);
    else if (tab === "settle") renderSettlementsTab(currentGroup);
  }
}

// ==========================================
// 5. MODAL CONTROLLERS & EVENT LISTENERS
// ==========================================

function openModal(modalId: string) {
  document.getElementById(modalId)?.classList.remove("hidden");
}

function closeModal(modalId: string) {
  document.getElementById(modalId)?.classList.add("hidden");
}

function openReimburseModal(fromId?: string, toId?: string, amount?: string) {
  if (!currentGroup) return;
  openModal("modal-record-reimbursement");

  const currSuffix = document.getElementById("reimburse-modal-currency-suffix");
  if (currSuffix) currSuffix.textContent = currentGroup.currency;

  const fromSelect = document.getElementById("select-reimburse-from") as HTMLSelectElement;
  const toSelect = document.getElementById("select-reimburse-to") as HTMLSelectElement;
  const amountInput = document.getElementById("input-reimburse-amount") as HTMLInputElement;
  const notesInput = document.getElementById("input-reimburse-notes") as HTMLInputElement;

  if (fromSelect && toSelect) {
    const options = currentGroup.participants
      .map((p) => `<option value="${p.id}">${p.name}</option>`)
      .join("");
    fromSelect.innerHTML = options;
    toSelect.innerHTML = options;

    if (fromId) fromSelect.value = fromId;
    if (toId) {
      toSelect.value = toId;
    } else if (currentGroup.participants.length > 1) {
      toSelect.selectedIndex = fromSelect.selectedIndex === 0 ? 1 : 0;
    }
  }

  if (amountInput) amountInput.value = amount || "";
  if (notesInput) notesInput.value = "";
}

function setupModalsAndForms() {
  // Close buttons
  for (const btn of document.querySelectorAll(".modal-close-btn")) {
    btn.addEventListener("click", () => {
      closeModal("modal-create-group");
      closeModal("modal-add-member");
      closeModal("modal-add-expense");
      closeModal("modal-record-reimbursement");
    });
  }

  // Open Create Group
  document
    .getElementById("open-create-group-btn")
    ?.addEventListener("click", () => openModal("modal-create-group"));
  document
    .getElementById("hero-create-group-btn")
    ?.addEventListener("click", () => openModal("modal-create-group"));
  document
    .getElementById("empty-create-group-btn")
    ?.addEventListener("click", () => openModal("modal-create-group"));

  // Add participant row inside create group form
  const participantContainer = document.getElementById("create-group-participants-container");
  document.getElementById("btn-add-participant-row")?.addEventListener("click", () => {
    if (!participantContainer) return;
    const row = document.createElement("div");
    row.className = "flex items-center gap-2";
    row.innerHTML = `
      <input type="text" placeholder="Participant name" required class="participant-name-input flex-1 px-3 py-2 rounded-xl bg-slate-950 border border-slate-800 text-sm text-slate-100 placeholder-slate-600 focus:outline-none focus:border-indigo-500" />
      <button type="button" class="btn-remove-row text-slate-500 hover:text-rose-400 p-1 cursor-pointer">✕</button>
    `;
    row.querySelector(".btn-remove-row")?.addEventListener("click", () => row.remove());
    participantContainer.appendChild(row);
  });

  // Handle Create Group submit
  document.getElementById("form-create-group")?.addEventListener("submit", async (e) => {
    e.preventDefault();
    const nameInput = document.getElementById("input-group-name") as HTMLInputElement;
    const currSelect = document.getElementById("select-group-currency") as HTMLSelectElement;
    const participantInputs = document.querySelectorAll(
      ".participant-name-input"
    ) as NodeListOf<HTMLInputElement>;

    const name = nameInput.value.trim();
    const currency = currSelect.value;
    const participants = Array.from(participantInputs)
      .map((i) => i.value.trim())
      .filter((n) => n.length > 0);

    if (name && participants.length > 0) {
      const newGroup = await api.createGroup(name, currency, participants);
      closeModal("modal-create-group");
      nameInput.value = "";
      openGroup(newGroup.id);
    }
  });

  // Open Add Member
  document.getElementById("btn-open-add-member")?.addEventListener("click", () => {
    openModal("modal-add-member");
    (document.getElementById("input-member-name") as HTMLInputElement)?.focus();
  });

  // Handle Add Member submit
  document.getElementById("form-add-member")?.addEventListener("submit", async (e) => {
    e.preventDefault();
    if (!currentGroup) return;
    const nameInput = document.getElementById("input-member-name") as HTMLInputElement;
    const name = nameInput.value.trim();
    if (name) {
      const updated = await api.addParticipant(currentGroup.id, name);
      currentGroup = updated;
      closeModal("modal-add-member");
      nameInput.value = "";
      openGroup(updated.id);
    }
  });

  // Open Add Expense
  const openExpenseModal = () => {
    if (!currentGroup) return;
    openModal("modal-add-expense");

    // Populate currency suffix
    const currSuffix = document.getElementById("expense-modal-currency-suffix");
    if (currSuffix) currSuffix.textContent = currentGroup.currency;

    // Populate payer dropdown
    const payerSelect = document.getElementById("select-expense-payer") as HTMLSelectElement;
    if (payerSelect) {
      payerSelect.innerHTML = currentGroup.participants
        .map((p) => `<option value="${p.id}">${p.name}</option>`)
        .join("");
    }

    // Populate split checkboxes
    const splitContainer = document.getElementById("expense-split-participants-container");
    if (splitContainer) {
      splitContainer.innerHTML = currentGroup.participants
        .map(
          (p) => `
          <label class="flex items-center gap-2 p-2 rounded-lg hover:bg-slate-900 cursor-pointer">
            <input type="checkbox" value="${p.id}" checked class="split-checkbox rounded bg-slate-900 border-slate-700 text-indigo-600 focus:ring-indigo-500 w-4 h-4 cursor-pointer" />
            <span class="text-xs font-medium text-slate-200">${p.name}</span>
          </label>
        `
        )
        .join("");
    }
  };

  document.getElementById("btn-open-add-expense")?.addEventListener("click", openExpenseModal);
  document.getElementById("btn-empty-add-expense")?.addEventListener("click", openExpenseModal);

  // Toggle select all split
  let allSelected = true;
  document.getElementById("btn-toggle-select-all-split")?.addEventListener("click", () => {
    allSelected = !allSelected;
    const checkboxes = document.querySelectorAll(".split-checkbox") as NodeListOf<HTMLInputElement>;
    for (const cb of checkboxes) {
      cb.checked = allSelected;
    }
    const toggleBtn = document.getElementById("btn-toggle-select-all-split");
    if (toggleBtn) toggleBtn.textContent = allSelected ? "Deselect All" : "Select All";
  });

  // Handle Add Expense submit
  document.getElementById("form-add-expense")?.addEventListener("submit", async (e) => {
    e.preventDefault();
    if (!currentGroup) return;

    const titleInput = document.getElementById("input-expense-title") as HTMLInputElement;
    const amountInput = document.getElementById("input-expense-amount") as HTMLInputElement;
    const payerSelect = document.getElementById("select-expense-payer") as HTMLSelectElement;
    const checkboxes = document.querySelectorAll(
      ".split-checkbox:checked"
    ) as NodeListOf<HTMLInputElement>;

    const title = titleInput.value.trim();
    const amountDecimal = Number.parseFloat(amountInput.value);
    const amountCents = Math.round(amountDecimal * 100);
    const paidBy = payerSelect.value;
    const splitAmong = Array.from(checkboxes).map((cb) => cb.value);

    if (title && amountCents > 0 && splitAmong.length > 0) {
      const updated = await api.addExpense(currentGroup.id, title, amountCents, paidBy, splitAmong);
      currentGroup = updated;
      closeModal("modal-add-expense");
      titleInput.value = "";
      amountInput.value = "";
      openGroup(updated.id);
    }
  });

  // Open Record Reimbursement buttons
  document.getElementById("btn-open-reimburse")?.addEventListener("click", () => {
    openReimburseModal();
  });
  document.getElementById("btn-open-reimburse-nav")?.addEventListener("click", () => {
    openReimburseModal();
  });

  // Handle Record Reimbursement submit
  document.getElementById("form-record-reimbursement")?.addEventListener("submit", async (e) => {
    e.preventDefault();
    if (!currentGroup) return;

    const fromSelect = document.getElementById("select-reimburse-from") as HTMLSelectElement;
    const toSelect = document.getElementById("select-reimburse-to") as HTMLSelectElement;
    const amountInput = document.getElementById("input-reimburse-amount") as HTMLInputElement;
    const notesInput = document.getElementById("input-reimburse-notes") as HTMLInputElement;

    const fromId = fromSelect.value;
    const toId = toSelect.value;
    const amountDecimal = Number.parseFloat(amountInput.value);
    const amountCents = Math.round(amountDecimal * 100);
    const notes = notesInput.value.trim();

    if (fromId === toId) {
      alert("The sender and recipient cannot be the same person.");
      return;
    }

    if (amountCents > 0) {
      const updated = await api.recordReimbursement(
        currentGroup.id,
        fromId,
        toId,
        amountCents,
        notes
      );
      currentGroup = updated;
      closeModal("modal-record-reimbursement");
      amountInput.value = "";
      notesInput.value = "";
      openGroup(updated.id);
    }
  });

  // Delete current group button
  document.getElementById("btn-delete-group")?.addEventListener("click", async () => {
    if (!currentGroup) return;
    if (confirm(`Are you sure you want to permanently delete "${currentGroup.name}"?`)) {
      await api.deleteGroup(currentGroup.id);
      loadDashboard();
    }
  });

  // Navigation back to Dashboard
  document.getElementById("nav-home-btn")?.addEventListener("click", loadDashboard);
  document.getElementById("btn-back-dashboard")?.addEventListener("click", loadDashboard);

  // Tabs switching
  document
    .getElementById("tab-btn-expenses")
    ?.addEventListener("click", () => switchTab("expenses"));
  document
    .getElementById("tab-btn-balances")
    ?.addEventListener("click", () => switchTab("balances"));
  document.getElementById("tab-btn-settle")?.addEventListener("click", () => switchTab("settle"));
}

// ==========================================
// 6. INITIALIZATION
// ==========================================
window.addEventListener("DOMContentLoaded", () => {
  setupModalsAndForms();
  loadDashboard();
});
