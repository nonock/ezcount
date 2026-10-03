// The logged-in account, and the recovery key to show once after signing up or recovering.
import { api } from "@/services/api";
import type { AccountInfo } from "@/types";

/** A recovery key to show once the user is in, and why it's new. */
export interface NewRecoveryKey {
  key: string;
  reason: "signup" | "recovered";
}

class Session {
  /** Undefined while loading, null when logged out. */
  account = $state<AccountInfo | null | undefined>(undefined);
  newRecoveryKey = $state<NewRecoveryKey | null>(null);

  get loggedIn(): boolean {
    return Boolean(this.account);
  }

  /** The name the user goes by: their profile's, or their username. */
  get name(): string {
    return this.account?.display_name ?? this.account?.username ?? "";
  }

  /** Whether the user put this group away. */
  isArchived(groupId: string): boolean {
    return this.account?.archived?.includes(groupId) ?? false;
  }

  /** Which participant the user is in a group, if they said. */
  identityIn(groupId: string): string | null {
    return this.account?.identities[groupId] ?? null;
  }

  async refresh() {
    try {
      this.account = await api.getAccount();
    } catch (err) {
      console.error("Failed to load the account:", err);
      this.account = null;
    }
  }
}

export const session = new Session();
