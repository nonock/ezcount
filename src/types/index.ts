export interface Participant {
  id: string;
  name: string;
}

export interface ExpenseSplit {
  participant_id: string;
  shares: number;
}

export interface ExpenseHistoryEntry {
  edited_at: string;
  previous_title: string;
  previous_amount_cents: number;
  previous_paid_by: string;
  previous_splits: ExpenseSplit[];
  summary: string;
}

export interface Expense {
  id: string;
  group_id: string;
  title: string;
  amount_cents: number;
  paid_by: string; // Participant ID
  splits: ExpenseSplit[];
  created_at: string; // ISO 8601 string
  updated_at?: string | null;
  history?: ExpenseHistoryEntry[];
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

export type TabType = "expenses" | "balances" | "settle";
