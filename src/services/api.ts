import { type Result, commands } from "../bindings";
import type {
  AccountInfo,
  ExpenseSplit,
  Group,
  NativeFeatures,
  ParticipantBalance,
  PasswordStrength,
  SettlementTransfer,
  SignedIn,
  SyncInfo,
} from "../types";

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

  /** Removes the group from the account, on all the user's devices. */
  async leaveGroup(groupId: string): Promise<void> {
    unwrap(await commands.leaveGroup(groupId));
  },

  async addParticipant(groupId: string, name: string): Promise<Group> {
    return unwrap(await commands.addParticipant(groupId, name));
  },

  async addExpense(
    groupId: string,
    title: string,
    amountCents: number,
    paidBy: string,
    splits: ExpenseSplit[],
    createdAt?: string | null
  ): Promise<Group> {
    return unwrap(
      await commands.addExpense(groupId, title, amountCents, paidBy, splits, createdAt || null)
    );
  },

  async updateExpense(
    groupId: string,
    expenseId: string,
    title: string,
    amountCents: number,
    paidBy: string,
    splits: ExpenseSplit[],
    createdAt?: string | null
  ): Promise<Group> {
    return unwrap(
      await commands.updateExpense(
        groupId,
        expenseId,
        title,
        amountCents,
        paidBy,
        splits,
        createdAt || null
      )
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

  async removeParticipant(groupId: string, participantId: string): Promise<Group> {
    return unwrap(await commands.removeParticipant(groupId, participantId));
  },

  async getStorageWarnings(): Promise<string[]> {
    return commands.getStorageWarnings();
  },

  async getSyncInfo(groupId: string): Promise<SyncInfo> {
    return unwrap(await commands.getSyncInfo(groupId));
  },

  async syncNow(groupId: string): Promise<SyncInfo> {
    return unwrap(await commands.syncNow(groupId));
  },

  async joinGroup(inviteCode: string): Promise<Group> {
    return unwrap(await commands.joinGroup(inviteCode));
  },

  async getAccount(): Promise<AccountInfo | null> {
    return unwrap(await commands.getAccount());
  },

  /** Also returns the new account's recovery key, when the relay supports them. */
  async signUp(serverUrl: string, username: string, password: string): Promise<SignedIn> {
    return unwrap(await commands.signUp(serverUrl, username, password));
  },

  /** Sets a new password with the recovery key and logs in; returns the next recovery key. */
  async recoverAccount(
    serverUrl: string,
    username: string,
    recoveryKey: string,
    newPassword: string
  ): Promise<SignedIn> {
    return unwrap(await commands.recoverAccount(serverUrl, username, recoveryKey, newPassword));
  },

  async changePassword(currentPassword: string, newPassword: string): Promise<void> {
    unwrap(await commands.changePassword(currentPassword, newPassword));
  },

  /** Replaces the recovery key; the old one stops working. */
  async replaceRecoveryKey(password: string): Promise<string> {
    return unwrap(await commands.replaceRecoveryKey(password));
  },

  async logIn(serverUrl: string, username: string, password: string): Promise<AccountInfo> {
    return unwrap(await commands.logIn(serverUrl, username, password));
  },

  /** Fails while changes are not uploaded yet, unless `force` is set. */
  async logOut(force = false): Promise<void> {
    unwrap(await commands.logOut(force));
  },

  async setIdentity(groupId: string, participantId: string): Promise<AccountInfo> {
    return unwrap(await commands.setIdentity(groupId, participantId));
  },

  async addSelf(groupId: string, name: string): Promise<Group> {
    return unwrap(await commands.addSelf(groupId, name));
  },

  async getBalances(groupId: string): Promise<ParticipantBalance[]> {
    return unwrap(await commands.getBalances(groupId));
  },

  async getSettlements(groupId: string): Promise<SettlementTransfer[]> {
    return unwrap(await commands.getSettlements(groupId));
  },

  /** How hard a new password is to guess; signing up requires `acceptable`. */
  async passwordStrength(password: string, username: string): Promise<PasswordStrength> {
    return commands.passwordStrength(password, username);
  },

  async nativeFeatures(): Promise<NativeFeatures> {
    return commands.nativeFeatures();
  },

  /** Opens the system share sheet; only where `nativeFeatures().share`. */
  async shareText(text: string, title: string): Promise<void> {
    unwrap(await commands.shareText(text, title));
  },
};
