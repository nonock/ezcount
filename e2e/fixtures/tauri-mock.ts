// In-memory Tauri IPC simulator for Playwright E2E tests
// Validates that all invocation signatures match the Rust tauri::command definitions.

export interface MockExpenseSplit {
  participant_id: string;
  shares: number;
}

export interface MockExpenseHistoryEntry {
  edited_at: string;
  previous_title: string;
  previous_amount_cents: number;
  previous_paid_by: string;
  previous_splits: MockExpenseSplit[];
  summary: string;
}

export interface MockExpense {
  id: string;
  group_id: string;
  title: string;
  amount_cents: number;
  paid_by: string;
  splits: MockExpenseSplit[];
  created_at: string;
  updated_at: string;
  history?: MockExpenseHistoryEntry[];
  is_reimbursement?: boolean;
}

export interface MockGroup {
  id: string;
  name: string;
  currency: string;
  participants: { id: string; name: string; removed?: boolean }[];
  expenses: MockExpense[];
  created_at: string;
}

export const MOCK_PASSWORD = "correct horse";
export const MOCK_SERVER = "http://localhost:8787";

/**
 * Seeds (set on `window` before the app loads):
 * - `__SEED_GROUPS__`: groups on the device; the user is their first participant
 *   unless `__SEED_IDENTITIES__` (group id -> participant id) says otherwise
 * - `__LOGGED_OUT__`: start on the login screen
 * - `__REMOTE_GROUPS__`: groups that can be joined with an invite code
 * - `__UNSYNCED__`: log out fails unless forced
 * - `__OPENED_WITH__`: the link the app was opened with (deep link)
 * - `__RECOVERY_KEY__`: the account's recovery key; new ones are `MOCK-KEY<n>-AAAA-…`
 * - `__NATIVE__`: `{ share, scan }` features, none by default; shared texts land in
 *   `window.__shared`
 * - `__SCANNED__`: what the camera "scans"
 * - `__STORAGE_WARNINGS__`
 */
export function installTauriMock() {
  // Serialized into the page, so it cannot use the module-level constants above.
  const MOCK_PASSWORD = "correct horse";
  const MOCK_SERVER = "http://localhost:8787";
  const w = window as any;
  let groups: MockGroup[] | null = null;
  let account: {
    username: string;
    server_url: string;
    identities: Record<string, string>;
  } | null = null;
  let accountLoaded = false;
  // The account's current password and recovery key; each new key is numbered.
  let password = MOCK_PASSWORD;
  // Seeds are set after this script runs, so the seeded key is read when first needed.
  let recoveryKey: string | null = null;
  const currentRecoveryKey = () => recoveryKey ?? w.__RECOVERY_KEY__ ?? "";
  let keysIssued = 0;
  function nextRecoveryKey() {
    keysIssued += 1;
    recoveryKey = `MOCK-KEY${keysIssued}-AAAA-BBBB-CCCC-DDDD-EEEE-FFFF`;
    return recoveryKey;
  }

  function getAccount() {
    if (!accountLoaded) {
      accountLoaded = true;
      if (!w.__LOGGED_OUT__) {
        const identities: Record<string, string> = {};
        for (const g of w.__SEED_GROUPS__ || []) {
          if (g.participants[0]) identities[g.id] = g.participants[0].id;
        }
        account = {
          username: "alice",
          server_url: MOCK_SERVER,
          identities: w.__SEED_IDENTITIES__ ? { ...w.__SEED_IDENTITIES__ } : identities,
        };
      }
    }
    return account;
  }

  function requireAccount() {
    const acc = getAccount();
    if (!acc) throw new Error("Log in first");
    return acc;
  }

  function getGroups(): MockGroup[] {
    if (!groups) {
      const seed = w.__SEED_GROUPS__;
      const seeded: MockGroup[] = seed && getAccount() ? JSON.parse(JSON.stringify(seed)) : [];
      groups = seeded;
      return seeded;
    }
    return groups;
  }

  // A stand-in for zxcvbn: longer is stronger, a few common passwords and the username are weak.
  function passwordStrength(password: string, username: string) {
    const pw = String(password || "");
    let score =
      pw.length < 8 ? 0 : pw.length < 10 ? 1 : pw.length < 12 ? 2 : pw.length < 16 ? 3 : 4;
    let warning: string | null = null;
    if (["password", "password123", "qwertyuiop"].includes(pw.toLowerCase())) {
      score = 0;
      warning = "This is a top-10 common password.";
    }
    const name = String(username || "")
      .trim()
      .toLowerCase();
    if (name && pw.toLowerCase().includes(name)) score = Math.min(score, 1);
    return {
      score,
      acceptable: pw.length >= 8 && score >= 3,
      warning,
      suggestions: score < 3 ? ["Add another word or two. Uncommon words are better."] : [],
    };
  }

  function checkCredentials(username: string, password: string, signingUp: boolean) {
    const name = String(username || "")
      .trim()
      .toLowerCase();
    if (!/^[a-z0-9][a-z0-9._-]{2,31}$/.test(name)) {
      throw new Error(
        signingUp
          ? "Usernames are 3 to 32 letters, digits, dots, dashes or underscores"
          : "Wrong username or password"
      );
    }
    if (signingUp && String(password).length < 8) {
      throw new Error("Use a password of at least 8 characters");
    }
    if (signingUp && !passwordStrength(password, name).acceptable) {
      throw new Error("This password is too easy to guess. Try a few unrelated words.");
    }
    return name;
  }

  function computeBalances(group: MockGroup) {
    const map = new Map<string, { paid: number; owed: number }>();
    for (const p of group.participants) {
      map.set(p.id, { paid: 0, owed: 0 });
    }

    for (const exp of group.expenses) {
      const payer = map.get(exp.paid_by);
      if (payer) payer.paid += exp.amount_cents;

      if (exp.splits && exp.splits.length > 0) {
        const totalShares = exp.splits.reduce((sum, s) => sum + s.shares, 0);
        if (totalShares > 0) {
          const allocated = exp.splits.map((s, idx) => {
            const base = Math.floor((exp.amount_cents * s.shares) / totalShares);
            const rem = (exp.amount_cents * s.shares) % totalShares;
            return { base, rem, idx, pid: s.participant_id };
          });

          const totalAllocated = allocated.reduce((sum, a) => sum + a.base, 0);
          const remainder = exp.amount_cents - totalAllocated;
          const order = [...allocated.keys()].sort((a, b) => allocated[b].rem - allocated[a].rem);
          for (let i = 0; i < remainder; i++) {
            allocated[order[i]].base += 1;
          }

          for (const item of allocated) {
            const debtor = map.get(item.pid);
            if (debtor) debtor.owed += item.base;
          }
        }
      }
    }

    // Mirrors engine.rs: removed participants only appear while they have something to settle.
    return group.participants
      .map((p) => {
        const data = map.get(p.id) || { paid: 0, owed: 0 };
        return {
          participant_id: p.id,
          participant_name: p.name,
          paid_cents: data.paid,
          owed_cents: data.owed,
          net_cents: data.paid - data.owed,
          removed: Boolean(p.removed),
        };
      })
      .filter((b) => !b.removed || b.net_cents !== 0);
  }

  function computeSettlements(group: MockGroup) {
    const balances = computeBalances(group);
    const debtors = balances
      .filter((b) => b.net_cents < 0)
      .map((b) => ({ ...b, net_cents: -b.net_cents }))
      .sort((a, b) => b.net_cents - a.net_cents);

    const creditors = balances
      .filter((b) => b.net_cents > 0)
      .map((b) => ({ ...b }))
      .sort((a, b) => b.net_cents - a.net_cents);

    const transfers: any[] = [];
    let d = 0;
    let c = 0;
    while (d < debtors.length && c < creditors.length) {
      const debtor = debtors[d];
      const creditor = creditors[c];
      const payment = Math.min(debtor.net_cents, creditor.net_cents);
      if (payment > 0) {
        transfers.push({
          from_id: debtor.participant_id,
          from_name: debtor.participant_name,
          to_id: creditor.participant_id,
          to_name: creditor.participant_name,
          amount_cents: payment,
        });
        debtor.net_cents -= payment;
        creditor.net_cents -= payment;
      }
      if (debtor.net_cents === 0) d++;
      if (creditor.net_cents === 0) c++;
    }
    return transfers;
  }

  function clone<T>(val: T): T {
    return JSON.parse(JSON.stringify(val));
  }

  // Sync state per group, and groups "on the relay" that can be joined (seeded by tests).
  const syncInfos = new Map<string, any>();
  // Every group of an account is shared.
  function syncInfo(groupId: string) {
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
  function inviteFor(serverUrl: string, groupId: string) {
    return `${serverUrl}/join#v=2&g=${groupId}&k=mock-key`;
  }

  // Minimal event plugin: `listen` registers a callback that tests can fire with
  // `window.__emitMockEvent(name, payload)`.
  let nextCallbackId = 1;
  const callbacks = new Map<number, (event: any) => void>();
  const listeners = new Map<string, number[]>();
  (window as any).__TAURI_EVENT_PLUGIN_INTERNALS__ = {
    unregisterListener: (event: string, id: number) => {
      listeners.set(
        event,
        (listeners.get(event) || []).filter((x) => x !== id)
      );
    },
  };
  w.__removeGroupElsewhere = (groupId: string) => {
    const idx = getGroups().findIndex((x) => x.id === groupId);
    if (idx !== -1) getGroups().splice(idx, 1);
  };
  (window as any).__emitMockEvent = (event: string, payload: unknown) => {
    for (const id of listeners.get(event) || []) {
      callbacks.get(id)?.({ event, id, payload });
    }
  };

  (window as any).__TAURI_INTERNALS__ = {
    transformCallback: (callback: (event: any) => void) => {
      const id = nextCallbackId++;
      callbacks.set(id, callback);
      return id;
    },
    invoke: async (cmd: string, args?: any) => {
      switch (cmd) {
        case "plugin:event|listen": {
          listeners.set(args.event, [...(listeners.get(args.event) || []), args.handler]);
          return args.handler;
        }

        case "plugin:event|unlisten":
          return null;

        case "plugin:deep-link|get_current":
          return w.__OPENED_WITH__ ? [w.__OPENED_WITH__] : null;

        case "password_strength":
          return passwordStrength(args.password, args.username);

        case "native_features":
          return { share: false, scan: false, ...w.__NATIVE__ };

        case "plugin:barcode-scanner|check_permissions":
          return { camera: "granted" };

        case "plugin:barcode-scanner|scan":
          return { content: w.__SCANNED__, format: "QR_CODE", bounds: null };

        case "share_text":
          w.__shared = [...(w.__shared || []), args.text];
          return null;

        case "get_storage_warnings":
          return clone((window as any).__STORAGE_WARNINGS__ || []);

        case "get_sync_info": {
          if (!getGroups().some((x) => x.id === args?.groupId)) throw new Error("Group not found");
          return clone(syncInfo(args.groupId));
        }

        case "get_account":
          return clone(getAccount());

        case "sign_up": {
          if (getAccount()) throw new Error("This device is already logged in");
          const username = checkCredentials(args.username, args.password, true);
          if ((w.__TAKEN_USERNAMES__ || []).includes(username)) {
            throw new Error(`The username "${username}" is already taken`);
          }
          account = { username, server_url: args.serverUrl, identities: {} };
          password = args.password;
          return { account: clone(account), recovery_key: nextRecoveryKey() };
        }

        case "log_in": {
          if (getAccount()) throw new Error("This device is already logged in");
          const username = checkCredentials(args.username, args.password, false);
          if (args.password !== password) throw new Error("Wrong username or password");
          account = { username, server_url: args.serverUrl, identities: {} };
          groups = [];
          return clone(account);
        }

        case "recover_account": {
          if (getAccount()) throw new Error("This device is already logged in");
          const username = checkCredentials(args.username, args.newPassword, true);
          const typed = String(args.recoveryKey).toUpperCase().replace(/[\s-]/g, "");
          if (!currentRecoveryKey() || typed !== currentRecoveryKey().replace(/-/g, "")) {
            throw new Error("Wrong username or recovery key");
          }
          account = { username, server_url: args.serverUrl, identities: {} };
          groups = [];
          password = args.newPassword;
          return { account: clone(account), recovery_key: nextRecoveryKey() };
        }

        case "change_password": {
          requireAccount();
          if (args.currentPassword !== password) throw new Error("Your current password is wrong");
          checkCredentials(getAccount()?.username ?? "", args.newPassword, true);
          password = args.newPassword;
          return null;
        }

        case "replace_recovery_key": {
          requireAccount();
          if (args.password !== password) throw new Error("Wrong password");
          return nextRecoveryKey();
        }

        case "log_out": {
          if (w.__UNSYNCED__ && !args.force) {
            throw new Error(
              "Some changes on this device are not uploaded yet. Connect to the internet and try again, or log out anyway and lose them."
            );
          }
          account = null;
          groups = [];
          return null;
        }

        case "set_identity": {
          const acc = requireAccount();
          const g = getGroups().find((x) => x.id === args?.groupId);
          if (!g) throw new Error("Group not found");
          if (!g.participants.some((x) => x.id === args.participantId && !x.removed)) {
            throw new Error("This person is not a member of the group");
          }
          acc.identities[args.groupId] = args.participantId;
          return clone(acc);
        }

        case "add_self": {
          const acc = requireAccount();
          const g = getGroups().find((x) => x.id === args?.groupId);
          if (!g) throw new Error("Group not found");
          const id = `p-self-${Date.now()}`;
          g.participants.push({ id, name: args.name });
          acc.identities[args.groupId] = id;
          return clone(g);
        }

        case "sync_now": {
          const info = { ...syncInfo(args.groupId), last_synced_at: new Date().toISOString() };
          syncInfos.set(args.groupId, info);
          return clone(info);
        }

        case "join_group": {
          // Invite links (<server>/join#g=…) and the ezcount://join?group=… form.
          const code = String(args?.inviteCode || "").trim();
          let server = "";
          let groupId = "";
          const link = code.match(/^(https?:\/\/.*)\/join#(.*)$/);
          if (link) {
            server = link[1];
            groupId = new URLSearchParams(link[2]).get("g") || "";
          } else if (code.startsWith("ezcount://join?")) {
            const params = new URL(code.replace("ezcount://", "https://")).searchParams;
            server = params.get("server") || "";
            groupId = params.get("group") || "";
          }
          if (!groupId) throw new Error("This is not a valid ezcount invite");
          requireAccount();
          if (getGroups().some((x) => x.id === groupId)) {
            throw new Error("This group is already in your account");
          }
          const remote = ((window as any).__REMOTE_GROUPS__ || []).find(
            (g: MockGroup) => g.id === groupId
          );
          if (!remote) throw new Error("The sync server does not know this group");
          getGroups().push(clone(remote));
          syncInfos.set(groupId, {
            group_id: groupId,
            enabled: true,
            server_url: server,
            invite_code: inviteFor(server, groupId),
            last_synced_at: new Date().toISOString(),
            last_error: null,
          });
          return clone(remote);
        }

        case "remove_participant": {
          const g = getGroups().find((x) => x.id === args?.groupId);
          if (!g) throw new Error("Group not found");
          const p = g.participants.find((x) => x.id === args?.participantId);
          if (!p) throw new Error("Participant not found");
          p.removed = true;
          return clone(g);
        }

        case "get_groups":
          return clone(getGroups());

        case "get_group": {
          if (!args || typeof args.groupId !== "string") {
            throw new Error("missing required argument `group_id`");
          }
          const g = getGroups().find((x) => x.id === args.groupId);
          if (!g) throw new Error("Group not found");
          return clone(g);
        }

        case "create_group": {
          if (!args || typeof args.name !== "string" || !Array.isArray(args.participants)) {
            throw new Error("invalid create_group arguments");
          }
          const acc = requireAccount();
          const now = new Date().toISOString();
          const newGroup: MockGroup = {
            id: `group-${Date.now()}`,
            name: args.name,
            currency: args.currency || "EUR",
            participants: args.participants.map((p: string, i: number) => ({
              id: `p-${i + 1}`,
              name: p,
            })),
            expenses: [],
            created_at: now,
          };
          getGroups().unshift(newGroup);
          acc.identities[newGroup.id] = newGroup.participants[0].id;
          return clone(newGroup);
        }

        case "leave_group": {
          if (!args || typeof args.groupId !== "string") {
            throw new Error("missing required argument `group_id`");
          }
          const idx = getGroups().findIndex((x) => x.id === args.groupId);
          if (idx !== -1) getGroups().splice(idx, 1);
          if (account) delete account.identities[args.groupId];
          return null;
        }

        case "add_participant": {
          const g = getGroups().find((x) => x.id === args?.groupId);
          if (!g) throw new Error("Group not found");
          g.participants.push({ id: `p-${Date.now()}`, name: args.name });
          return clone(g);
        }

        case "add_expense": {
          const g = getGroups().find((x) => x.id === args?.groupId);
          if (!g) throw new Error("Group not found");
          const now = new Date().toISOString();
          const createdAt = args?.createdAt || now;
          const exp: MockExpense = {
            id: `exp-${Date.now()}`,
            group_id: args.groupId,
            title: args.title,
            amount_cents: args.amountCents,
            paid_by: args.paidBy,
            splits: args.splits,
            created_at: createdAt,
            updated_at: now,
            history: [],
            is_reimbursement: false,
          };
          g.expenses.unshift(exp);
          return clone(g);
        }

        case "update_expense": {
          const g = getGroups().find((x) => x.id === args?.groupId);
          if (!g) throw new Error("Group not found");
          const exp = g.expenses.find((x) => x.id === args?.expenseId);
          if (!exp) throw new Error("Expense not found");

          const prevTitle = exp.title;
          const prevAmount = exp.amount_cents;
          const prevPayer = exp.paid_by;
          const prevSplits = [...exp.splits];

          exp.history = exp.history || [];
          exp.history.push({
            edited_at: new Date().toISOString(),
            previous_title: prevTitle,
            previous_amount_cents: prevAmount,
            previous_paid_by: prevPayer,
            previous_splits: prevSplits,
            summary: `Amount changed to ${args.amountCents / 100} • Title updated to ${args.title}`,
          });

          exp.title = args.title;
          exp.amount_cents = args.amountCents;
          exp.paid_by = args.paidBy;
          exp.splits = args.splits;
          if (args?.createdAt) {
            exp.created_at = args.createdAt;
          }
          exp.updated_at = new Date().toISOString();
          return clone(g);
        }

        case "delete_expense": {
          const g = getGroups().find((x) => x.id === args?.groupId);
          if (!g) throw new Error("Group not found");
          g.expenses = g.expenses.filter((x) => x.id !== args?.expenseId);
          return clone(g);
        }

        case "record_reimbursement": {
          const g = getGroups().find((x) => x.id === args?.groupId);
          if (!g) throw new Error("Group not found");
          const fromP = g.participants.find((p) => p.id === args.fromId)?.name || "Unknown";
          const toP = g.participants.find((p) => p.id === args.toId)?.name || "Unknown";
          const now = new Date().toISOString();
          g.expenses.unshift({
            id: `exp-${Date.now()}`,
            group_id: args.groupId,
            title: `Payment: ${fromP} → ${toP}`,
            amount_cents: args.amountCents,
            paid_by: args.fromId,
            splits: [{ participant_id: args.toId, shares: 1 }],
            created_at: now,
            updated_at: now,
            history: [],
            is_reimbursement: true,
          });
          return clone(g);
        }

        case "get_balances": {
          const g = getGroups().find((x) => x.id === args?.groupId);
          if (!g) throw new Error("Group not found");
          return clone(computeBalances(g));
        }

        case "get_settlements": {
          const g = getGroups().find((x) => x.id === args?.groupId);
          if (!g) throw new Error("Group not found");
          return clone(computeSettlements(g));
        }

        default:
          throw new Error(`Unknown command: ${cmd}`);
      }
    },
  };
}
