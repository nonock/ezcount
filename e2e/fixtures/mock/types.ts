// The groups and the account as the mock keeps them, shaped like the core's models.

export interface MockExpenseSplit {
  participant_id: string;
  shares: number;
  fixed_cents?: number | null;
}

export interface MockOriginalAmount {
  currency: string;
  amount_cents: number;
  rate: string;
}

export interface MockExpenseHistoryEntry {
  edited_at: string;
  previous_title: string;
  previous_category?: string | null;
  previous_amount_cents: number;
  previous_paid_by: string;
  previous_payers?: { participant_id: string; amount_cents: number }[];
  previous_splits: MockExpenseSplit[];
  previous_original?: MockOriginalAmount | null;
  summary: string;
  edited_by?: string | null;
}

export interface MockExpense {
  id: string;
  group_id: string;
  title: string;
  category?: string | null;
  amount_cents: number;
  original?: MockOriginalAmount | null;
  paid_by: string;
  payers?: { participant_id: string; amount_cents: number }[];
  splits: MockExpenseSplit[];
  created_at: string;
  updated_at: string;
  history?: MockExpenseHistoryEntry[];
  is_reimbursement?: boolean;
  income?: boolean;
  added_at?: string | null;
  added_by?: string | null;
  recurring?: string | null;
  items?: { name: string; amount_cents: number; participants: string[] }[];
  comments?: { id: string; text: string; created_at: string; by?: string | null }[];
}

export interface MockRecurringExpense {
  id: string;
  title: string;
  category?: string | null;
  amount_cents: number;
  income?: boolean;
  paid_by: string;
  payers?: { participant_id: string; amount_cents: number }[];
  splits: MockExpenseSplit[];
  every: string;
  // The first one's day, and how many were added (none when left out): `next` follows.
  start: string;
  made?: number;
  next?: string;
  paused?: boolean;
  added_by?: string | null;
}

export interface MockGroup {
  id: string;
  name: string;
  description?: string;
  image?: string | null;
  currency: string;
  participants: {
    id: string;
    name: string;
    removed?: boolean;
    avatar?: string | null;
    iban?: string | null;
    added_at?: string | null;
    added_by?: string | null;
    removed_at?: string | null;
    removed_by?: string | null;
  }[];
  expenses: MockExpense[];
  created_at: string;
  deleted?: boolean;
  deletion_votes?: string[];
  trash?: { expense: MockExpense; deleted_at: string; deleted_by?: string | null }[];
  recurring?: MockRecurringExpense[];
  /** Changed by a newer version of the app: only its name shows. */
  needs_update?: boolean;
}

export interface MockAccount {
  username: string;
  server_url: string;
  display_name: string | null;
  avatar: string | null;
  iban: string | null;
  archived: string[];
  identities: Record<string, string>;
  update_required?: boolean;
}

/** A command, with the arguments the app's bindings send it. */
export type Commands = Record<string, (args: any) => unknown>;
