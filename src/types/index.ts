export interface Participant {
  id: string;
  name: string;
}

export interface Expense {
  id: string;
  group_id: string;
  title: string;
  amount_cents: number;
  paid_by: string; // Participant ID
  split_among: string[]; // List of Participant IDs
  created_at: string; // ISO 8601 string
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
