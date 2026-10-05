// The account: signing up, logging in and out, the profile, and handing a login or an invite
// to another device.

import { checkCredentials, checkIban, checkPicture, passwordStrength } from "../checks";
import {
  clone,
  complete,
  currentRecoveryKey,
  findGroup,
  getAccount,
  getGroups,
  newAccount,
  nextRecoveryKey,
  requireAccount,
  state,
  w,
} from "../state";
import type { Commands } from "../types";

/** Gives a member the profile's name, picture and IBAN, like `show_profile` in sync.rs. */
function showProfile(groupId: string, participantId: string) {
  const member = getGroups()
    .find((g) => g.id === groupId)
    ?.participants.find((p) => p.id === participantId);
  const account = state.account;
  if (!member || !account) return;
  if (account.display_name) member.name = account.display_name;
  member.avatar = account.avatar;
  member.iban = account.iban;
}

function mustBeLoggedOut() {
  if (getAccount()) throw new Error("This device is already logged in");
}

/** What an `ezcount://…` link carries. */
const paramsOf = (link: unknown) =>
  new URL(String(link).replace("ezcount://", "https://")).searchParams;

export const accountCommands: Commands = {
  password_strength: (args) => passwordStrength(args.password, args.username),

  get_account: () => clone(getAccount()),

  sign_up(args) {
    mustBeLoggedOut();
    const username = checkCredentials(args.username, args.password, true);
    if ((w.__TAKEN_USERNAMES__ || []).includes(username)) {
      throw new Error(`The username "${username}" is already taken`);
    }
    state.account = newAccount(username, args.serverUrl);
    state.password = args.password;
    return {
      account: clone(state.account),
      recovery_key: w.__OLD_RELAY__ ? null : nextRecoveryKey(),
    };
  },

  log_in(args) {
    mustBeLoggedOut();
    const username = checkCredentials(args.username, args.password, false);
    if (args.password !== state.password) throw new Error("Wrong username or password");
    state.account = newAccount(username, args.serverUrl);
    state.groups = [];
    return clone(state.account);
  },

  recover_account(args) {
    mustBeLoggedOut();
    const username = checkCredentials(args.username, args.newPassword, true);
    const typed = String(args.recoveryKey).toUpperCase().replace(/[\s-]/g, "");
    if (!currentRecoveryKey() || typed !== currentRecoveryKey().replace(/-/g, "")) {
      throw new Error("Wrong username or recovery key");
    }
    state.account = newAccount(username, args.serverUrl);
    state.groups = [];
    state.password = args.newPassword;
    return { account: clone(state.account), recovery_key: nextRecoveryKey() };
  },

  change_password(args) {
    requireAccount();
    if (args.currentPassword !== state.password) {
      throw new Error("Your current password is wrong");
    }
    checkCredentials(getAccount()?.username ?? "", args.newPassword, true);
    state.password = args.newPassword;
    return null;
  },

  replace_recovery_key(args) {
    requireAccount();
    if (args.password !== state.password) throw new Error("Wrong password");
    return nextRecoveryKey();
  },

  create_login_link(args) {
    const acc = requireAccount();
    if (args.password !== state.password) throw new Error("Wrong password");
    return {
      link: `ezcount://login?server=${encodeURIComponent(acc.server_url)}&code=mock-code`,
      expires_in: w.__LINK_SECONDS__ ?? 120,
    };
  },

  receive_link(args) {
    if (w.__OLD_RELAY__) {
      throw new Error("This sync server can't pass things between devices yet");
    }
    state.receiveAsked = 0;
    return `ezcount://receive?server=${encodeURIComponent(args.serverUrl)}&code=mock-${Date.now()}&for=${args.purpose}`;
  },

  receive(args) {
    state.receiveAsked += 1;
    if (w.__PHONE_SCANS__ == null || state.receiveAsked < w.__PHONE_SCANS__) return null;
    const params = paramsOf(args.link);
    if (params.get("for") === "login") {
      mustBeLoggedOut();
      state.account = newAccount("alice", params.get("server") || "");
      state.groups = [];
      return { account: clone(state.account), group: null };
    }
    requireAccount();
    const remote = (w.__REMOTE_GROUPS__ || [])[0];
    if (!remote) throw new Error("Group not found");
    const joined = complete(clone(remote));
    getGroups().push(joined);
    return { account: null, group: clone(joined) };
  },

  send_login(args) {
    requireAccount();
    if (!String(args.link).includes("for=login")) {
      throw new Error("This code is for joining a group: scan it from the group's Invite window");
    }
    if (args.password !== state.password) throw new Error("Wrong password");
    w.__sent = [...(w.__sent || []), { login: true }];
    return null;
  },

  send_group_invite(args) {
    requireAccount();
    if (!String(args.link).startsWith("ezcount://receive?")) {
      throw new Error("This is not a code shown by ezcount to receive something");
    }
    if (!String(args.link).includes("for=group")) {
      throw new Error("This code is for logging in: scan it from Connect a device");
    }
    w.__sent = [...(w.__sent || []), { group: args.groupId }];
    return null;
  },

  log_in_with_link(args) {
    mustBeLoggedOut();
    const link = String(args.link);
    if (!link.startsWith("ezcount://login?")) {
      throw new Error("This is not an ezcount login code");
    }
    const params = paramsOf(link);
    if (params.get("code") !== "mock-code") {
      throw new Error("This code has expired or was already used. Show a new one and scan it.");
    }
    state.account = newAccount("alice", params.get("server") || "");
    state.groups = [];
    return clone(state.account);
  },

  log_out(args) {
    if (w.__UNSYNCED__ && !args.force) {
      throw new Error(
        "Some changes on this device are not uploaded yet. Connect to the internet and try again, or log out anyway and lose them."
      );
    }
    state.account = null;
    state.groups = [];
    return null;
  },

  set_identity(args) {
    const acc = requireAccount();
    const g = findGroup(args?.groupId);
    if (!g.participants.some((x) => x.id === args.participantId && !x.removed)) {
      throw new Error("This person is not a member of the group");
    }
    const previous = g.participants.find((x) => x.id === acc.identities[args.groupId]);
    if (previous && previous.id !== args.participantId) {
      previous.avatar = null;
      previous.iban = null;
    }
    acc.identities[args.groupId] = args.participantId;
    showProfile(args.groupId, args.participantId);
    return clone(acc);
  },

  update_profile(args) {
    const acc = requireAccount();
    const name = String(args.name).trim();
    if (name.length > 50) throw new Error("This name is too long (50 characters at most)");
    checkPicture(args.avatar);
    const iban = args.iban?.trim() ? checkIban(args.iban) : null;
    acc.display_name = name || null;
    acc.avatar = args.avatar ?? null;
    acc.iban = iban;
    for (const [groupId, participantId] of Object.entries(acc.identities)) {
      showProfile(groupId, participantId);
    }
    return clone(acc);
  },

  add_self(args) {
    const acc = requireAccount();
    const g = findGroup(args?.groupId);
    const id = `p-self-${Date.now()}`;
    g.participants.push({
      id,
      name: args.name,
      added_at: new Date().toISOString(),
      added_by: id,
    });
    acc.identities[args.groupId] = id;
    showProfile(args.groupId, id);
    return clone(g);
  },
};
