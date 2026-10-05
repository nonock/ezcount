// What the mock remembers while a page lives: the account, its groups and their sync state.
// Seeds are set on `window` after the mock is added, so they are read when first needed.

import { MOCK_PASSWORD, MOCK_SERVER } from "./constants";
import type { MockAccount, MockGroup } from "./types";

export const w = window as any;

export const state = {
  groups: null as MockGroup[] | null,
  account: null as MockAccount | null,
  accountLoaded: false,
  /** The account's current password. */
  password: MOCK_PASSWORD,
  /** Its recovery key, once a new one was issued; each new key is numbered. */
  recoveryKey: null as string | null,
  keysIssued: 0,
  /** How many times the code this device shows was asked about. */
  receiveAsked: 0,
};

export function clone<T>(val: T): T {
  return JSON.parse(JSON.stringify(val));
}

export const currentRecoveryKey = (): string => state.recoveryKey ?? w.__RECOVERY_KEY__ ?? "";

export function nextRecoveryKey() {
  state.keysIssued += 1;
  state.recoveryKey = `MOCK-KEY${state.keysIssued}-AAAA-BBBB-CCCC-DDDD-EEEE-FFFF`;
  return state.recoveryKey;
}

/** An account that just logged in or signed up: no profile, no groups yet. */
export function newAccount(username: string, serverUrl: string): MockAccount {
  return {
    username,
    server_url: serverUrl,
    display_name: null,
    avatar: null,
    iban: null,
    archived: [],
    identities: {},
  };
}

export function getAccount() {
  if (!state.accountLoaded) {
    state.accountLoaded = true;
    if (!w.__LOGGED_OUT__) {
      const identities: Record<string, string> = {};
      for (const g of w.__SEED_GROUPS__ || []) {
        if (g.participants[0]) identities[g.id] = g.participants[0].id;
      }
      state.account = {
        username: "alice",
        server_url: MOCK_SERVER,
        display_name: w.__PROFILE__?.display_name ?? null,
        avatar: w.__PROFILE__?.avatar ?? null,
        iban: w.__PROFILE__?.iban ?? null,
        archived: [...(w.__ARCHIVED__ ?? [])],
        identities: w.__SEED_IDENTITIES__ ? { ...w.__SEED_IDENTITIES__ } : identities,
        update_required: Boolean(w.__UPDATE_REQUIRED__),
      };
    }
  }
  return state.account;
}

export function requireAccount() {
  const acc = getAccount();
  if (!acc) throw new Error("Log in first");
  return acc;
}

/** A group as the app expects one, with what a seed may leave out. */
export function complete(group: MockGroup): MockGroup {
  group.description ??= "";
  group.image ??= null;
  group.trash ??= [];
  group.recurring ??= [];
  return group;
}

export function getGroups(): MockGroup[] {
  if (!state.groups) {
    const seed = w.__SEED_GROUPS__;
    const seeded: MockGroup[] = seed && getAccount() ? clone(seed) : [];
    state.groups = seeded.map(complete);
  }
  return state.groups;
}

/** The group a command is about. */
export function findGroup(groupId: unknown): MockGroup {
  const group = getGroups().find((x) => x.id === groupId);
  if (!group) throw new Error("Group not found");
  return group;
}

/** The participant the user is in a group. */
export function me(group: MockGroup) {
  return getAccount()?.identities[group.id] ?? null;
}

// Sync state per group, and groups "on the relay" that can be joined (seeded by tests).
export const syncInfos = new Map<string, any>();

export function inviteFor(serverUrl: string, groupId: string) {
  return `${serverUrl}/join#v=2&g=${groupId}&k=mock-key`;
}

/** Every group of an account is shared. */
export function syncInfo(groupId: string) {
  return (
    syncInfos.get(groupId) || {
      group_id: groupId,
      enabled: true,
      server_url: MOCK_SERVER,
      invite_code: inviteFor(MOCK_SERVER, groupId),
      last_synced_at: null,
      last_error: null,
    }
  );
}
