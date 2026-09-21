import { invoke } from "@tauri-apps/api/core";
import type { ExpenseSplit, Group, ParticipantBalance, SettlementTransfer } from "../types";

export const api = {
  async getGroups(): Promise<Group[]> {
    return invoke<Group[]>("get_groups");
  },

  async getGroup(id: string): Promise<Group> {
    return invoke<Group>("get_group", { id });
  },

  async createGroup(name: string, currency: string, participants: string[]): Promise<Group> {
    return invoke<Group>("create_group", { name, currency, participants });
  },

  async deleteGroup(id: string): Promise<void> {
    return invoke<void>("delete_group", { id });
  },

  async addParticipant(groupId: string, name: string): Promise<Group> {
    return invoke<Group>("add_participant", { groupId, name });
  },

  async addExpense(
    groupId: string,
    title: string,
    amountCents: number,
    paidBy: string,
    splits: ExpenseSplit[]
  ): Promise<Group> {
    return invoke<Group>("add_expense", {
      groupId,
      title,
      amountCents,
      paidBy,
      splits,
    });
  },

  async updateExpense(
    groupId: string,
    expenseId: string,
    title: string,
    amountCents: number,
    paidBy: string,
    splits: ExpenseSplit[]
  ): Promise<Group> {
    return invoke<Group>("update_expense", {
      groupId,
      expenseId,
      title,
      amountCents,
      paidBy,
      splits,
    });
  },

  async deleteExpense(groupId: string, expenseId: string): Promise<Group> {
    return invoke<Group>("delete_expense", { groupId, expenseId });
  },

  async recordReimbursement(
    groupId: string,
    fromId: string,
    toId: string,
    amountCents: number,
    notes?: string
  ): Promise<Group> {
    return invoke<Group>("record_reimbursement", {
      groupId,
      fromId,
      toId,
      amountCents,
      notes: notes || null,
    });
  },

  async getBalances(groupId: string): Promise<ParticipantBalance[]> {
    return invoke<ParticipantBalance[]>("get_balances", { groupId });
  },

  async getSettlements(groupId: string): Promise<SettlementTransfer[]> {
    return invoke<SettlementTransfer[]>("get_settlements", { groupId });
  },
};
