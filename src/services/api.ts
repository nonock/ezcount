import { type Result, commands } from "../bindings";
import type { ExpenseSplit, Group, ParticipantBalance, SettlementTransfer } from "../types";

function unwrap<T>(result: Result<T, string>): T {
  if (result.status === "ok") return result.data;
  throw new Error(result.error);
}

export const api = {
  async getGroups(): Promise<Group[]> {
    return commands.getGroups();
  },

  async getGroup(groupId: string): Promise<Group> {
    return unwrap(await commands.getGroup(groupId));
  },

  async createGroup(name: string, currency: string, participants: string[]): Promise<Group> {
    return unwrap(await commands.createGroup(name, currency, participants));
  },

  async deleteGroup(groupId: string): Promise<void> {
    unwrap(await commands.deleteGroup(groupId));
  },

  async addParticipant(groupId: string, name: string): Promise<Group> {
    return unwrap(await commands.addParticipant(groupId, name));
  },

  async addExpense(
    groupId: string,
    title: string,
    amountCents: number,
    paidBy: string,
    splits: ExpenseSplit[]
  ): Promise<Group> {
    return unwrap(await commands.addExpense(groupId, title, amountCents, paidBy, splits));
  },

  async updateExpense(
    groupId: string,
    expenseId: string,
    title: string,
    amountCents: number,
    paidBy: string,
    splits: ExpenseSplit[]
  ): Promise<Group> {
    return unwrap(
      await commands.updateExpense(groupId, expenseId, title, amountCents, paidBy, splits)
    );
  },

  async deleteExpense(groupId: string, expenseId: string): Promise<Group> {
    return unwrap(await commands.deleteExpense(groupId, expenseId));
  },

  async recordReimbursement(
    groupId: string,
    fromId: string,
    toId: string,
    amountCents: number,
    notes?: string
  ): Promise<Group> {
    return unwrap(
      await commands.recordReimbursement(groupId, fromId, toId, amountCents, notes || null)
    );
  },

  async getBalances(groupId: string): Promise<ParticipantBalance[]> {
    return unwrap(await commands.getBalances(groupId));
  },

  async getSettlements(groupId: string): Promise<SettlementTransfer[]> {
    return unwrap(await commands.getSettlements(groupId));
  },
};
