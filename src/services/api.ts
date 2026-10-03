import { commands, type Result } from "../bindings";
import type {
  AccountInfo,
  ExpensePayer,
  ExpenseSplit,
  Group,
  LoginLink,
  NativeFeatures,
  OriginalAmount,
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

  /** Creates a group from the text of a CSV file, as `exportGroupCsv` writes them. */
  async importGroupCsv(name: string, csv: string): Promise<Group> {
    return unwrap(await commands.importGroupCsv(name, csv));
  },

  /** The group as CSV text: a line per expense, a column per person. */
  async exportGroupCsv(groupId: string): Promise<string> {
    return unwrap(await commands.exportGroupCsv(groupId));
  },

  /**
   * The relay's exchange rate from one currency to another on a day (`YYYY-MM-DD`), to
   * suggest; null when it has none.
   */
  async suggestExchangeRate(from: string, to: string, date: string | null): Promise<string | null> {
    return unwrap(await commands.suggestExchangeRate(from, to, date));
  },

  /** Removes the group from the account, on all the user's devices. */
  async leaveGroup(groupId: string): Promise<void> {
    unwrap(await commands.leaveGroup(groupId));
  },

  /** Renames the group and sets its currency; amounts are not converted. */
  async updateGroup(
    groupId: string,
    name: string,
    currency: string,
    description: string,
    image: string | null
  ): Promise<Group> {
    return unwrap(await commands.updateGroup(groupId, name, currency, description, image));
  },

  async addParticipant(groupId: string, name: string): Promise<Group> {
    return unwrap(await commands.addParticipant(groupId, name));
  },

  async renameParticipant(groupId: string, participantId: string, name: string): Promise<Group> {
    return unwrap(await commands.renameParticipant(groupId, participantId, name));
  },

  async addExpense(
    groupId: string,
    title: string,
    amountCents: number,
    paidBy: string,
    payers: ExpensePayer[],
    splits: ExpenseSplit[],
    createdAt?: string | null,
    original?: OriginalAmount | null
  ): Promise<Group> {
    return unwrap(
      await commands.addExpense(
        groupId,
        title,
        amountCents,
        paidBy,
        payers,
        splits,
        createdAt || null,
        original ?? null
      )
    );
  },

  async updateExpense(
    groupId: string,
    expenseId: string,
    title: string,
    amountCents: number,
    paidBy: string,
    payers: ExpensePayer[],
    splits: ExpenseSplit[],
    createdAt?: string | null,
    original?: OriginalAmount | null
  ): Promise<Group> {
    return unwrap(
      await commands.updateExpense(
        groupId,
        expenseId,
        title,
        amountCents,
        paidBy,
        payers,
        splits,
        createdAt || null,
        original ?? null
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

  /**
   * A link that logs another device into the account, to show as a QR code. It works once,
   * for `expires_in` seconds.
   */
  async createLoginLink(password: string): Promise<LoginLink> {
    return unwrap(await commands.createLoginLink(password));
  },

  /** Logs in with a link scanned from a device that is logged in already. */
  async logInWithLink(link: string): Promise<AccountInfo> {
    return unwrap(await commands.logInWithLink(link));
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

  async updateProfile(name: string, avatar: string | null): Promise<AccountInfo> {
    return unwrap(await commands.updateProfile(name, avatar));
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

  /** Writes a file into the Downloads folder and returns its path; only where `nativeFeatures().save`. */
  async saveDownload(fileName: string, text: string): Promise<string> {
    return unwrap(await commands.saveDownload(fileName, text));
  },

  /** Opens the system share sheet; only where `nativeFeatures().share`. */
  async shareText(text: string, title: string): Promise<void> {
    unwrap(await commands.shareText(text, title));
  },
};
