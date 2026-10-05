// Groups: reading, making, joining, leaving and deleting them, their members and their
// balances.

import { checkPicture } from "../checks";
import { computeBalances, computeSettlements, splitAmount } from "../engine";
import { keepUp } from "../recurring";
import {
  clone,
  findGroup,
  getAccount,
  getGroups,
  inviteFor,
  requireAccount,
  state,
  syncInfo,
  syncInfos,
  w,
} from "../state";
import type { Commands, MockGroup } from "../types";

const CSV_COLUMNS =
  "Date,Title,Amount,Currency,Paid by,Type,Original amount,Original currency,Exchange rate,Split";

const money = (cents: number) => (cents / 100).toFixed(2);

export const groupCommands: Commands = {
  get_sync_info(args) {
    findGroup(args?.groupId);
    return clone(syncInfo(args.groupId));
  },

  sync_now(args) {
    const info = { ...syncInfo(args.groupId), last_synced_at: new Date().toISOString() };
    syncInfos.set(args.groupId, info);
    return clone(info);
  },

  join_group(args) {
    // Invite links (<server>/join#g=…) and the ezcount://join?group=… form.
    const code = String(args?.inviteCode || "").trim();
    let server = "";
    let groupId = "";
    const link = code.match(/^(https?:\/\/.*)\/join#(.*)$/);
    if (link) {
      server = link[1];
      groupId = new URLSearchParams(link[2]).get("g") || "";
    } else if (code.startsWith("ezcount://join?")) {
      const params = new URL(code.replace("ezcount://", "https://")).searchParams;
      server = params.get("server") || "";
      groupId = params.get("group") || "";
    }
    if (!groupId) throw new Error("This is not a valid ezcount invite");
    requireAccount();
    if (getGroups().some((x) => x.id === groupId)) {
      throw new Error("This group is already in your account");
    }
    const remote = (w.__REMOTE_GROUPS__ || []).find((g: MockGroup) => g.id === groupId);
    if (!remote) throw new Error("The sync server does not know this group");
    getGroups().push(clone(remote));
    syncInfos.set(groupId, {
      group_id: groupId,
      enabled: true,
      server_url: server,
      invite_code: inviteFor(server, groupId),
      last_synced_at: new Date().toISOString(),
      last_error: null,
    });
    return clone(remote);
  },

  get_groups() {
    getGroups().forEach(keepUp);
    return clone(getGroups());
  },

  get_group(args) {
    if (!args || typeof args.groupId !== "string") {
      throw new Error("missing required argument `group_id`");
    }
    const g = findGroup(args.groupId);
    keepUp(g);
    return clone(g);
  },

  create_group(args) {
    if (!args || typeof args.name !== "string" || !Array.isArray(args.participants)) {
      throw new Error("invalid create_group arguments");
    }
    const acc = requireAccount();
    const now = new Date().toISOString();
    const newGroup: MockGroup = {
      id: `group-${Date.now()}`,
      name: args.name,
      description: "",
      image: null,
      currency: args.currency || "EUR",
      participants: args.participants.map((p: string, i: number) => ({
        id: `p-${i + 1}`,
        name: p,
        avatar: i === 0 ? acc.avatar : null,
      })),
      expenses: [],
      created_at: now,
      trash: [],
      recurring: [],
    };
    getGroups().unshift(newGroup);
    acc.identities[newGroup.id] = newGroup.participants[0].id;
    return clone(newGroup);
  },

  // The format of csv_file.rs, without its leniency: commas, no quoted cells, no Split
  // column, and the amounts owed become the shares.
  import_group_csv(args) {
    requireAccount();
    const [header, ...lines] = String(args.csv).replace(/^﻿/, "").trim().split(/\r?\n/);
    const columns = header.split(",");
    if (columns.slice(0, 10).join(",") !== CSV_COLUMNS) {
      throw new Error(
        "This is not an ezcount CSV file: its first line should be Date, Title, Amount, Currency, Paid by, Type, Original amount, Original currency, Exchange rate, Split, then one column per person"
      );
    }
    const id = `group-${Date.now()}`;
    const participants = columns.slice(10).map((name, i) => ({ id: `p-${i + 1}`, name }));
    const toCents = (text: string) => Math.round(Number(text) * 100);
    const now = new Date().toISOString();
    const imported: MockGroup = {
      id,
      name: args.name,
      currency: lines[0]?.split(",")[3] || "EUR",
      participants,
      expenses: lines.map((line, i) => {
        const cells = line.split(",");
        const payer = participants.find((p) => p.name === cells[4]);
        if (!payer) throw new Error(`Line ${i + 2}: ${cells[4]} paid, but has no column`);
        return {
          id: `exp-${i + 1}`,
          group_id: id,
          title: cells[1],
          amount_cents: toCents(cells[2]),
          original: cells[7]
            ? { currency: cells[7], amount_cents: toCents(cells[6]), rate: cells[8] }
            : null,
          paid_by: payer.id,
          splits: participants
            .map((p, column) => ({
              participant_id: p.id,
              shares: toCents(cells[10 + column] || "0"),
            }))
            .filter((split) => split.shares > 0),
          created_at: new Date(cells[0]).toISOString(),
          updated_at: now,
          history: [],
          is_reimbursement: cells[5] === "payment",
          income: cells[5] === "income",
        };
      }),
      created_at: now,
      trash: [],
      recurring: [],
    };
    getGroups().unshift(imported);
    return clone(imported);
  },

  export_group_csv(args) {
    const g = findGroup(args?.groupId);
    const nameOf = (id: string) => g.participants.find((p) => p.id === id)?.name ?? "";
    const lines = g.expenses.map((e) => {
      const owed = splitAmount(e);
      return [
        e.created_at,
        e.title,
        money(e.amount_cents),
        g.currency,
        nameOf(e.paid_by),
        e.is_reimbursement ? "payment" : e.income ? "income" : "expense",
        e.original ? money(e.original.amount_cents) : "",
        e.original?.currency ?? "",
        e.original?.rate ?? "",
        e.is_reimbursement
          ? ""
          : g.participants
              .map((p) => {
                const s = e.splits.find((x) => x.participant_id === p.id);
                return !s ? "-" : s.fixed_cents != null ? money(s.fixed_cents) : s.shares;
              })
              .join(" "),
        ...g.participants.map((p) => {
          const part = owed.find((o) => o.pid === p.id);
          return part ? money(part.base) : "";
        }),
      ].join(",");
    });
    const header = [CSV_COLUMNS, ...g.participants.map((p) => p.name)].join(",");
    return `${[header, ...lines].join("\n")}\n`;
  },

  leave_group(args) {
    if (!args || typeof args.groupId !== "string") {
      throw new Error("missing required argument `group_id`");
    }
    const idx = getGroups().findIndex((x) => x.id === args.groupId);
    if (idx !== -1) getGroups().splice(idx, 1);
    const account = state.account;
    if (account) {
      delete account.identities[args.groupId];
      account.archived = account.archived.filter((id) => id !== args.groupId);
    }
    return null;
  },

  // Like `delete_or_vote` in doc.rs: settled balances delete at once, otherwise every member
  // has to agree.
  delete_group(args) {
    const acc = requireAccount();
    const groups = getGroups();
    const g = findGroup(args?.groupId);
    const members = g.participants.filter((p) => !p.removed);
    if (!computeBalances(g).every((b) => b.net_cents === 0)) {
      const me = acc.identities[g.id];
      if (!members.some((p) => p.id === me)) {
        throw new Error("Say who you are in this group before asking to delete it");
      }
      g.deletion_votes = [...new Set([...(g.deletion_votes ?? []), me])];
      if (!members.every((p) => g.deletion_votes?.includes(p.id))) return clone(g);
    }
    groups.splice(groups.indexOf(g), 1);
    delete acc.identities[g.id];
    acc.archived = acc.archived.filter((id) => id !== g.id);
    return null;
  },

  refuse_group_deletion(args) {
    const g = findGroup(args?.groupId);
    g.deletion_votes = [];
    return clone(g);
  },

  set_group_archived(args) {
    const acc = requireAccount();
    findGroup(args?.groupId);
    acc.archived = acc.archived.filter((id) => id !== args.groupId);
    if (args.archived) acc.archived.push(args.groupId);
    return clone(acc);
  },

  update_group(args) {
    const g = findGroup(args?.groupId);
    const name = String(args.name).trim();
    const currency = String(args.currency).trim().toUpperCase();
    if (!name) throw new Error("Group name cannot be empty");
    if (!/^[A-Z]{3}$/.test(currency)) {
      throw new Error("The currency must be a three-letter code, such as EUR");
    }
    const description = String(args.description ?? "").trim();
    if (description.length > 500) {
      throw new Error("This description is too long (500 characters at most)");
    }
    checkPicture(args.image);
    g.name = name;
    g.currency = currency;
    g.description = description;
    g.image = args.image ?? null;
    return clone(g);
  },

  add_participant(args) {
    const g = findGroup(args?.groupId);
    g.participants.push({
      id: `p-${Date.now()}`,
      name: args.name,
      added_at: new Date().toISOString(),
      added_by: getAccount()?.identities[g.id] ?? null,
    });
    return clone(g);
  },

  rename_participant(args) {
    const g = findGroup(args?.groupId);
    const p = g.participants.find((x) => x.id === args?.participantId);
    if (!p) throw new Error("Participant not found");
    const name = String(args.name).trim();
    if (!name) throw new Error("Participant name cannot be empty");
    // Payments still titled as recorded get the new name, like in doc.rs.
    const nameOf = (id: string, renamed: boolean) =>
      renamed && id === p.id ? name : g.participants.find((x) => x.id === id)?.name;
    for (const e of g.expenses) {
      const to = e.splits[0]?.participant_id;
      if (!e.is_reimbursement || e.splits.length !== 1 || ![e.paid_by, to].includes(p.id)) {
        continue;
      }
      const before = `Payment: ${nameOf(e.paid_by, false)} → ${nameOf(to, false)}`;
      const notes = e.title.startsWith(before) ? e.title.slice(before.length) : null;
      if (notes === null || (notes && !notes.startsWith(" ("))) continue;
      e.title = `Payment: ${nameOf(e.paid_by, true)} → ${nameOf(to, true)}${notes}`;
    }
    p.name = name;
    return clone(g);
  },

  remove_participant(args) {
    const g = findGroup(args?.groupId);
    const p = g.participants.find((x) => x.id === args?.participantId);
    if (!p) throw new Error("Participant not found");
    p.removed = true;
    p.removed_at = new Date().toISOString();
    p.removed_by = getAccount()?.identities[g.id] ?? null;
    return clone(g);
  },

  get_balances: (args) => clone(computeBalances(findGroup(args?.groupId))),

  get_settlements: (args) => clone(computeSettlements(findGroup(args?.groupId))),
};
