// Groups and expenses for the unit tests, with only what a test cares about spelled out.

import type { AccountInfo, Expense, ExpenseSplit, Group, Participant } from "@/types";

/** Alice's account, in no group yet. */
export function account(more: Partial<AccountInfo> = {}): AccountInfo {
  return {
    username: "alice",
    server_url: "https://relay.example.com",
    display_name: null,
    avatar: null,
    archived: [],
    identities: {},
    ...more,
  };
}

export function participant(id: string, more: Partial<Participant> = {}): Participant {
  return { id, name: id.charAt(0).toUpperCase() + id.slice(1), ...more };
}

/** One part each for `ids`. */
export function equally(...ids: string[]): ExpenseSplit[] {
  return ids.map((participant_id) => ({ participant_id, shares: 1 }));
}

export function expense(more: Partial<Expense> = {}): Expense {
  return {
    id: "e1",
    group_id: "g1",
    title: "Taxi",
    amount_cents: 3000,
    paid_by: "alice",
    splits: equally("alice", "bob"),
    created_at: "2026-03-01T10:00:00Z",
    updated_at: "2026-03-01T10:00:00Z",
    ...more,
  };
}

/** A group of Alice, Bob and Carol, in euros. */
export function group(more: Partial<Group> = {}): Group {
  return {
    id: "g1",
    name: "Trip",
    currency: "EUR",
    participants: [participant("alice"), participant("bob"), participant("carol")],
    expenses: [],
    created_at: "2026-01-01T00:00:00Z",
    ...more,
  };
}
