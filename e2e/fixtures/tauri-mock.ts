// In-memory Tauri IPC simulator for Playwright E2E tests
// Validates that all invocation signatures match the Rust tauri::command definitions.

export interface MockExpenseSplit {
  participant_id: string;
  shares: number;
  fixed_cents?: number | null;
}

export interface MockOriginalAmount {
  currency: string;
  amount_cents: number;
  rate: string;
}

export interface MockExpenseHistoryEntry {
  edited_at: string;
  previous_title: string;
  previous_category?: string | null;
  previous_amount_cents: number;
  previous_paid_by: string;
  previous_payers?: { participant_id: string; amount_cents: number }[];
  previous_splits: MockExpenseSplit[];
  previous_original?: MockOriginalAmount | null;
  summary: string;
  edited_by?: string | null;
}

export interface MockExpense {
  id: string;
  group_id: string;
  title: string;
  category?: string | null;
  amount_cents: number;
  original?: MockOriginalAmount | null;
  paid_by: string;
  payers?: { participant_id: string; amount_cents: number }[];
  splits: MockExpenseSplit[];
  created_at: string;
  updated_at: string;
  history?: MockExpenseHistoryEntry[];
  is_reimbursement?: boolean;
  income?: boolean;
  added_at?: string | null;
  added_by?: string | null;
  recurring?: string | null;
  items?: { name: string; amount_cents: number; participants: string[] }[];
  comments?: { id: string; text: string; created_at: string; by?: string | null }[];
}

export interface MockRecurringExpense {
  id: string;
  title: string;
  category?: string | null;
  amount_cents: number;
  income?: boolean;
  paid_by: string;
  payers?: { participant_id: string; amount_cents: number }[];
  splits: MockExpenseSplit[];
  every: string;
  // The first one's day, and how many were added (none when left out): `next` follows.
  start: string;
  made?: number;
  next?: string;
  paused?: boolean;
  added_by?: string | null;
}

export interface MockGroup {
  id: string;
  name: string;
  description?: string;
  image?: string | null;
  currency: string;
  participants: {
    id: string;
    name: string;
    removed?: boolean;
    avatar?: string | null;
    iban?: string | null;
    added_at?: string | null;
    added_by?: string | null;
    removed_at?: string | null;
    removed_by?: string | null;
  }[];
  expenses: MockExpense[];
  created_at: string;
  deleted?: boolean;
  deletion_votes?: string[];
  trash?: { expense: MockExpense; deleted_at: string; deleted_by?: string | null }[];
  recurring?: MockRecurringExpense[];
}

export const MOCK_PASSWORD = "correct horse";
export const MOCK_SERVER = "http://localhost:8787";

/**
 * Seeds (set on `window` before the app loads):
 * - `__SEED_GROUPS__`: groups on the device; the user is their first participant
 *   unless `__SEED_IDENTITIES__` (group id -> participant id) says otherwise. A group's
 *   `recurring` expenses need only their `start`: the ones due are added when it is read
 * - `__LOGGED_OUT__`: start on the login screen
 * - `__REMOTE_GROUPS__`: groups that can be joined with an invite code
 * - `__UNSYNCED__`: log out fails unless forced
 * - `__OPENED_WITH__`: the link the app was opened with (deep link)
 * - `__RECOVERY_KEY__`: the account's recovery key; new ones are `MOCK-KEY<n>-AAAA-…`
 * - `__OLD_RELAY__`: sign-up gets no recovery key, like on a relay from before them
 * - `__NATIVE__`: `{ share, scan, save }` features, none by default; shared texts land in
 *   `window.__shared`, saved files in `window.__saved` (with `text`, or `bytes` for a PDF)
 * - `__SCANNED__`: what the camera "scans"
 * - messages sent from the feedback form land in `window.__feedback`; with `__OLD_RELAY__`
 *   the relay doesn't take them
 * - `__LINK_SECONDS__`: how long a login link works, 120 by default
 * - `__PHONE_SCANS__`: a phone scans the code this device shows (to log in, or to join the
 *   first of `__REMOTE_GROUPS__`) once it has been asked for this many times; never when
 *   left out. What this device sent to a code it scanned lands in `window.__sent`
 * - `__RATES__`: exchange rates the relay suggests, as `{ "USD/EUR": "0.9234" }`
 * - `__ARCHIVED__`: ids of the groups the user archived
 * - `__PROFILE__`: the account's `{ display_name, avatar, iban }`
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
    display_name: string | null;
    avatar: string | null;
    iban: string | null;
    archived: string[];
    identities: Record<string, string>;
  } | null = null;
  let accountLoaded = false;
  // The account's current password and recovery key; each new key is numbered.
  let password = MOCK_PASSWORD;
  // Seeds are set after this script runs, so the seeded key is read when first needed.
  let recoveryKey: string | null = null;
  const currentRecoveryKey = () => recoveryKey ?? w.__RECOVERY_KEY__ ?? "";
  let keysIssued = 0;
  // How many times the code this device shows was asked about.
  let receiveAsked = 0;
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
          display_name: w.__PROFILE__?.display_name ?? null,
          avatar: w.__PROFILE__?.avatar ?? null,
          iban: w.__PROFILE__?.iban ?? null,
          archived: [...(w.__ARCHIVED__ ?? [])],
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
      for (const group of seeded) {
        group.description ??= "";
        group.image ??= null;
        group.trash ??= [];
        group.recurring ??= [];
      }
      groups = seeded;
      return seeded;
    }
    return groups;
  }

  /** The participant the user is in a group. */
  function me(group: MockGroup) {
    return getAccount()?.identities[group.id] ?? null;
  }

  // The day of occurrence `n` of a repeated expense, like doc.rs's `occurrence`: the day of
  // the month is kept, or the month's last.
  function occurrence(start: string, every: string, n: number) {
    const date = new Date(start);
    if (every === "week") {
      date.setUTCDate(date.getUTCDate() + 7 * n);
      return date.toISOString();
    }
    const day = date.getUTCDate();
    date.setUTCDate(1);
    date.setUTCMonth(date.getUTCMonth() + (every === "year" ? 12 * n : n));
    const last = new Date(Date.UTC(date.getUTCFullYear(), date.getUTCMonth() + 1, 0)).getUTCDate();
    date.setUTCDate(Math.min(day, last));
    return date.toISOString();
  }

  // Adds the repeated expenses whose day has come, like doc.rs's `add_due_expenses`.
  function addDueExpenses(group: MockGroup) {
    const now = new Date().toISOString();
    const member = (id: string) => group.participants.some((p) => p.id === id && !p.removed);
    for (const model of group.recurring ?? []) {
      model.made ??= 0;
      model.next = occurrence(model.start, model.every, model.made);
      model.paused = ![
        model.paid_by,
        ...(model.payers ?? []).map((p) => p.participant_id),
        ...model.splits.map((s) => s.participant_id),
      ].every(member);
      while (!model.paused && model.next <= now) {
        const id = `${model.id}-${model.next.slice(0, 10).replaceAll("-", "")}`;
        const taken = [...group.expenses, ...(group.trash ?? []).map((d) => d.expense)];
        if (!taken.some((e) => e.id === id)) {
          group.expenses.unshift({
            id,
            group_id: group.id,
            title: model.title,
            category: model.category ?? null,
            amount_cents: model.amount_cents,
            income: model.income ?? false,
            paid_by: model.paid_by,
            payers: model.payers ?? [],
            splits: model.splits,
            created_at: model.next,
            updated_at: model.next,
            history: [],
            is_reimbursement: false,
            added_at: model.next,
            added_by: model.added_by ?? null,
            recurring: model.id,
          });
        }
        model.made += 1;
        model.next = occurrence(model.start, model.every, model.made);
      }
    }
  }

  // What reading a group does first, like api.rs's `keep_up`: the repeated expenses due are
  // added, and what was deleted more than 30 days ago leaves the trash.
  function keepUp(group: MockGroup) {
    addDueExpenses(group);
    const kept = new Date(Date.now() - 30 * 24 * 3600 * 1000).toISOString();
    group.trash = (group.trash ?? []).filter((d) => d.deleted_at > kept);
  }

  // An IBAN as stored, like doc.rs's `check_iban`.
  function checkIban(typed: string) {
    const iban = typed.replace(/\s/g, "").toUpperCase();
    let rest = 0;
    for (const c of iban.slice(4) + iban.slice(0, 4)) {
      const n = Number.parseInt(c, 36);
      rest = (n > 9 ? rest * 100 + n : rest * 10 + n) % 97;
    }
    if (!/^[A-Z]{2}\d{2}[A-Z0-9]{11,30}$/.test(iban) || rest !== 1) {
      throw new Error("This IBAN is not valid");
    }
    return iban;
  }

  // The splits of an expense entered line by line, like doc.rs's `items_splits`.
  function itemsSplits(amountCents: number, original: MockOriginalAmount | null, items: any[]) {
    const money = (cents: number) => (cents / 100).toFixed(2);
    const paid = original?.amount_cents ?? amountCents;
    const total = items.reduce((sum, item) => sum + item.amount_cents, 0);
    if (items.some((item) => item.participants.length === 0)) {
      throw new Error("Each item needs at least one person");
    }
    if (total !== paid) {
      throw new Error(`The items add up to ${money(total)}, not the expense's ${money(paid)}`);
    }
    const owed = new Map<string, number>();
    for (const item of items) {
      const people = item.participants.length;
      const each = Math.floor(item.amount_cents / people);
      const extra = item.amount_cents % people;
      item.participants.forEach((id: string, i: number) => {
        owed.set(id, (owed.get(id) ?? 0) + each + (i < extra ? 1 : 0));
      });
    }
    return [...owed]
      .filter(([, cents]) => cents > 0)
      .map(([participant_id, cents]) => ({ participant_id, shares: 0, fixed_cents: cents }));
  }

  // A stand-in for zxcvbn: longer is stronger, a few common passwords and the username are weak.
  const PICTURE = /^data:image\/(jpeg|png|webp);base64,[A-Za-z0-9+/=]+$/;
  function checkPicture(picture: string | null | undefined) {
    if (picture == null) return;
    if (!PICTURE.test(picture)) {
      throw new Error("This picture can't be used: pick a JPEG, PNG or WebP image");
    }
    if (picture.length > 200_000) throw new Error("This picture is too big");
  }

  /** Gives a member the profile's name, picture and IBAN, like `show_profile` in sync.rs. */
  function showProfile(groupId: string, participantId: string) {
    const member = getGroups()
      .find((g) => g.id === groupId)
      ?.participants.find((p) => p.id === participantId);
    if (!member || !account) return;
    if (account.display_name) member.name = account.display_name;
    member.avatar = account.avatar;
    member.iban = account.iban;
  }

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

  // `amount` in proportion to `weights`, like engine.rs's `split_weighted`.
  function splitWeighted(amount: number, weights: number[]) {
    const total = weights.reduce((sum, w) => sum + w, 0);
    if (total <= 0) return weights.map(() => 0);
    const allocated = weights.map((w) => ({
      base: Math.floor((amount * w) / total),
      rem: (amount * w) % total,
    }));
    const remainder = amount - allocated.reduce((sum, a) => sum + a.base, 0);
    const order = [...allocated.keys()].sort((a, b) => allocated[b].rem - allocated[a].rem);
    for (let i = 0; i < remainder; i++) {
      allocated[order[i]].base += 1;
    }
    return allocated.map((a) => a.base);
  }

  // What each person owes of an expense, like engine.rs's `owed`: fixed amounts first, the
  // parts share the rest, all in the currency the expense was paid in.
  function splitAmount(exp: MockExpense) {
    const splits = exp.splits || [];
    const shares = splits.map((s) => s.shares);
    let owed: number[];
    if (splits.every((s) => s.fixed_cents == null)) {
      owed = splitWeighted(exp.amount_cents, shares);
    } else {
      const paid = exp.original?.amount_cents ?? exp.amount_cents;
      const fixed = splits.reduce((sum, s) => sum + (s.fixed_cents ?? 0), 0);
      const rest = splitWeighted(Math.max(paid - fixed, 0), shares);
      const there = splits.map((s, i) => s.fixed_cents ?? rest[i]);
      owed = exp.original ? splitWeighted(exp.amount_cents, there) : there;
    }
    return splits.map((s, i) => ({ pid: s.participant_id, base: owed[i] }));
  }

  // The checks of doc.rs's `check_amounts` that the form can run into.
  function checkSplits(
    amountCents: number,
    original: MockOriginalAmount | null,
    splits: MockExpenseSplit[]
  ) {
    const money = (cents: number) => (cents / 100).toFixed(2);
    const paid = original?.amount_cents ?? amountCents;
    const fixed = splits.reduce((sum, s) => sum + (s.fixed_cents ?? 0), 0);
    if (fixed > paid) {
      throw new Error(
        `The fixed amounts add up to ${money(fixed)}, more than the expense's ${money(paid)}`
      );
    }
    if (splits.every((s) => s.fixed_cents != null) && fixed !== paid) {
      throw new Error(`The amounts add up to ${money(fixed)}, not the expense's ${money(paid)}`);
    }
  }

  /** Who paid, as `PaidBy::new` and `check_payers` in doc.rs: who paid the most first. */
  function paidBy(args: any) {
    const payers = [...(args.payers ?? [])].sort((a, b) => b.amount_cents - a.amount_cents);
    if (payers.length < 2) {
      return { paid_by: payers[0]?.participant_id ?? args.paidBy, payers: [] };
    }
    if (payers.some((p) => p.amount_cents <= 0)) {
      throw new Error("What each payer paid must be above zero");
    }
    const total = payers.reduce((sum, p) => sum + p.amount_cents, 0);
    const paid = args.original?.amount_cents ?? args.amountCents;
    const money = (cents: number) => (cents / 100).toFixed(2);
    if (total !== paid) {
      throw new Error(
        `The payers paid ${money(total)} between them, not the expense's ${money(paid)}`
      );
    }
    return { paid_by: payers[0].participant_id, payers };
  }

  /** The commands take the expense as one `expense` argument; the cases below read its fields. */
  function expenseArgs(args: any) {
    const e = args.expense;
    return {
      ...args,
      title: e.title,
      category: e.category || null,
      amountCents: e.amount_cents,
      paidBy: e.paid_by,
      payers: e.payers ?? [],
      splits: e.splits,
      createdAt: e.created_at ?? null,
      original: e.original ?? null,
      income: e.income ?? false,
      repeat: e.repeat || null,
      items: (e.items ?? []).map((item: any) => ({ ...item, name: item.name.trim() })),
    };
  }

  function computeBalances(group: MockGroup) {
    const map = new Map<string, { paid: number; owed: number }>();
    for (const p of group.participants) {
      map.set(p.id, { paid: 0, owed: 0 });
    }

    for (const exp of group.expenses) {
      // Money that came in counts the other way.
      const sign = exp.income ? -1 : 1;
      // Like `engine::paid`: several payers' amounts are in the currency paid.
      const payers = exp.payers ?? [];
      const there = payers.map((p) => p.amount_cents);
      const here = exp.original ? splitWeighted(exp.amount_cents, there) : there;
      const paid = payers.length
        ? payers.map((p, i) => ({ id: p.participant_id, cents: here[i] }))
        : [{ id: exp.paid_by, cents: exp.amount_cents }];
      for (const { id, cents } of paid) {
        const payer = map.get(id);
        if (payer) payer.paid += sign * cents;
      }

      for (const item of splitAmount(exp)) {
        const debtor = map.get(item.pid);
        if (debtor) debtor.owed += sign * item.base;
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
          return { share: false, scan: false, save: false, ...w.__NATIVE__ };

        case "plugin:app|version":
          return "0.0.0-test";

        case "send_feedback":
          requireAccount();
          if (w.__OLD_RELAY__) throw new Error("This sync server doesn't take messages yet");
          w.__feedback = [...(w.__feedback || []), args];
          return null;

        case "plugin:barcode-scanner|check_permissions":
          return { camera: "granted" };

        case "plugin:barcode-scanner|scan":
          return { content: w.__SCANNED__, format: "QR_CODE", bounds: null };

        case "save_download":
          w.__saved = [...(w.__saved || []), { name: args.fileName, text: args.text }];
          return `C:\\Users\\alice\\Downloads\\${args.fileName}`;

        case "save_file":
          w.__saved = [...(w.__saved || []), { name: args.fileName, bytes: args.data }];
          return `C:\\Users\\alice\\Downloads\\${args.fileName}`;

        case "share_file":
          w.__shared = [...(w.__shared || []), args.fileName];
          return null;

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
          account = {
            username,
            server_url: args.serverUrl,
            display_name: null,
            avatar: null,
            iban: null,
            archived: [],
            identities: {},
          };
          password = args.password;
          return {
            account: clone(account),
            recovery_key: w.__OLD_RELAY__ ? null : nextRecoveryKey(),
          };
        }

        case "log_in": {
          if (getAccount()) throw new Error("This device is already logged in");
          const username = checkCredentials(args.username, args.password, false);
          if (args.password !== password) throw new Error("Wrong username or password");
          account = {
            username,
            server_url: args.serverUrl,
            display_name: null,
            avatar: null,
            iban: null,
            archived: [],
            identities: {},
          };
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
          account = {
            username,
            server_url: args.serverUrl,
            display_name: null,
            avatar: null,
            iban: null,
            archived: [],
            identities: {},
          };
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

        case "create_login_link": {
          const acc = requireAccount();
          if (args.password !== password) throw new Error("Wrong password");
          return {
            link: `ezcount://login?server=${encodeURIComponent(acc.server_url)}&code=mock-code`,
            expires_in: w.__LINK_SECONDS__ ?? 120,
          };
        }

        case "receive_link": {
          if (w.__OLD_RELAY__)
            throw new Error("This sync server can't pass things between devices yet");
          receiveAsked = 0;
          return `ezcount://receive?server=${encodeURIComponent(args.serverUrl)}&code=mock-${Date.now()}&for=${args.purpose}`;
        }

        case "receive": {
          receiveAsked += 1;
          if (w.__PHONE_SCANS__ == null || receiveAsked < w.__PHONE_SCANS__) return null;
          const params = new URL(String(args.link).replace("ezcount://", "https://")).searchParams;
          if (params.get("for") === "login") {
            if (getAccount()) throw new Error("This device is already logged in");
            account = {
              username: "alice",
              server_url: params.get("server") || "",
              display_name: null,
              avatar: null,
              iban: null,
              archived: [],
              identities: {},
            };
            groups = [];
            return { account: clone(account), group: null };
          }
          requireAccount();
          const remote = (w.__REMOTE_GROUPS__ || [])[0];
          if (!remote) throw new Error("Group not found");
          const joined = JSON.parse(JSON.stringify(remote));
          joined.description ??= "";
          joined.image ??= null;
          joined.trash ??= [];
          joined.recurring ??= [];
          getGroups().push(joined);
          return { account: null, group: clone(joined) };
        }

        case "send_login": {
          requireAccount();
          if (!String(args.link).includes("for=login")) {
            throw new Error(
              "This code is for joining a group: scan it from the group's Invite window"
            );
          }
          if (args.password !== password) throw new Error("Wrong password");
          w.__sent = [...(w.__sent || []), { login: true }];
          return null;
        }

        case "send_group_invite": {
          requireAccount();
          if (!String(args.link).startsWith("ezcount://receive?")) {
            throw new Error("This is not a code shown by ezcount to receive something");
          }
          if (!String(args.link).includes("for=group")) {
            throw new Error("This code is for logging in: scan it from Connect a device");
          }
          w.__sent = [...(w.__sent || []), { group: args.groupId }];
          return null;
        }

        case "log_in_with_link": {
          if (getAccount()) throw new Error("This device is already logged in");
          const link = String(args.link);
          if (!link.startsWith("ezcount://login?")) {
            throw new Error("This is not an ezcount login code");
          }
          const params = new URL(link.replace("ezcount://", "https://")).searchParams;
          if (params.get("code") !== "mock-code") {
            throw new Error(
              "This code has expired or was already used. Show a new one and scan it."
            );
          }
          account = {
            username: "alice",
            server_url: params.get("server") || "",
            display_name: null,
            avatar: null,
            iban: null,
            archived: [],
            identities: {},
          };
          groups = [];
          return clone(account);
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
          const previous = g.participants.find((x) => x.id === acc.identities[args.groupId]);
          if (previous && previous.id !== args.participantId) {
            previous.avatar = null;
            previous.iban = null;
          }
          acc.identities[args.groupId] = args.participantId;
          showProfile(args.groupId, args.participantId);
          return clone(acc);
        }

        case "update_profile": {
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
        }

        case "add_self": {
          const acc = requireAccount();
          const g = getGroups().find((x) => x.id === args?.groupId);
          if (!g) throw new Error("Group not found");
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
          p.removed_at = new Date().toISOString();
          p.removed_by = getAccount()?.identities[g.id] ?? null;
          return clone(g);
        }

        case "get_groups":
          getGroups().forEach(keepUp);
          return clone(getGroups());

        case "get_group": {
          if (!args || typeof args.groupId !== "string") {
            throw new Error("missing required argument `group_id`");
          }
          const g = getGroups().find((x) => x.id === args.groupId);
          if (!g) throw new Error("Group not found");
          keepUp(g);
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
            description: "",
            image: null,
            currency: args.currency || "EUR",
            participants: args.participants.map((p: string, i: number) => ({
              id: `p-${i + 1}`,
              name: p,
              avatar: i === 0 ? acc.avatar : null,
            })),
            expenses: [],
            created_at: now,
            trash: [],
            recurring: [],
          };
          getGroups().unshift(newGroup);
          acc.identities[newGroup.id] = newGroup.participants[0].id;
          return clone(newGroup);
        }

        // The format of csv_file.rs, without its leniency: commas, no quoted cells, no
        // Split column, and the amounts owed become the shares.
        case "import_group_csv": {
          requireAccount();
          const [header, ...lines] = String(args.csv)
            .replace(/^\ufeff/, "")
            .trim()
            .split(/\r?\n/);
          const columns = header.split(",");
          if (
            columns.slice(0, 10).join(",") !==
            "Date,Title,Amount,Currency,Paid by,Type,Original amount,Original currency,Exchange rate,Split"
          ) {
            throw new Error(
              "This is not an ezcount CSV file: its first line should be Date, Title, Amount, Currency, Paid by, Type, Original amount, Original currency, Exchange rate, Split, then one column per person"
            );
          }
          const id = `group-${Date.now()}`;
          const participants = columns.slice(10).map((name, i) => ({ id: `p-${i + 1}`, name }));
          const toCents = (text: string) => Math.round(Number(text) * 100);
          const now = new Date().toISOString();
          const imported: MockGroup = {
            id,
            name: args.name,
            currency: lines[0]?.split(",")[3] || "EUR",
            participants,
            expenses: lines.map((line, i) => {
              const cells = line.split(",");
              const payer = participants.find((p) => p.name === cells[4]);
              if (!payer) throw new Error(`Line ${i + 2}: ${cells[4]} paid, but has no column`);
              return {
                id: `exp-${i + 1}`,
                group_id: id,
                title: cells[1],
                amount_cents: toCents(cells[2]),
                original: cells[7]
                  ? { currency: cells[7], amount_cents: toCents(cells[6]), rate: cells[8] }
                  : null,
                paid_by: payer.id,
                splits: participants
                  .map((p, column) => ({
                    participant_id: p.id,
                    shares: toCents(cells[10 + column] || "0"),
                  }))
                  .filter((split) => split.shares > 0),
                created_at: new Date(cells[0]).toISOString(),
                updated_at: now,
                history: [],
                is_reimbursement: cells[5] === "payment",
                income: cells[5] === "income",
              };
            }),
            created_at: now,
            trash: [],
            recurring: [],
          };
          getGroups().unshift(imported);
          return clone(imported);
        }

        case "export_group_csv": {
          const g = getGroups().find((x) => x.id === args?.groupId);
          if (!g) throw new Error("Group not found");
          const money = (cents: number) => (cents / 100).toFixed(2);
          const nameOf = (id: string) => g.participants.find((p) => p.id === id)?.name ?? "";
          const lines = g.expenses.map((e) => {
            const owed = splitAmount(e);
            return [
              e.created_at,
              e.title,
              money(e.amount_cents),
              g.currency,
              nameOf(e.paid_by),
              e.is_reimbursement ? "payment" : e.income ? "income" : "expense",
              e.original ? money(e.original.amount_cents) : "",
              e.original?.currency ?? "",
              e.original?.rate ?? "",
              e.is_reimbursement
                ? ""
                : g.participants
                    .map((p) => {
                      const s = e.splits.find((x) => x.participant_id === p.id);
                      return !s ? "-" : s.fixed_cents != null ? money(s.fixed_cents) : s.shares;
                    })
                    .join(" "),
              ...g.participants.map((p) => {
                const part = owed.find((o) => o.pid === p.id);
                return part ? money(part.base) : "";
              }),
            ].join(",");
          });
          const header = [
            "Date,Title,Amount,Currency,Paid by,Type,Original amount,Original currency,Exchange rate,Split",
            ...g.participants.map((p) => p.name),
          ].join(",");
          return `${[header, ...lines].join("\n")}\n`;
        }

        case "suggest_exchange_rate":
          return w.__RATES__?.[`${args.from}/${args.to}`] ?? null;

        case "leave_group": {
          if (!args || typeof args.groupId !== "string") {
            throw new Error("missing required argument `group_id`");
          }
          const idx = getGroups().findIndex((x) => x.id === args.groupId);
          if (idx !== -1) getGroups().splice(idx, 1);
          if (account) {
            delete account.identities[args.groupId];
            account.archived = account.archived.filter((id) => id !== args.groupId);
          }
          return null;
        }

        // Like `delete_or_vote` in doc.rs: settled balances delete at once, otherwise every
        // member has to agree.
        case "delete_group": {
          const acc = requireAccount();
          const groups = getGroups();
          const g = groups.find((x) => x.id === args?.groupId);
          if (!g) throw new Error("Group not found");
          const members = g.participants.filter((p) => !p.removed);
          if (!computeBalances(g).every((b) => b.net_cents === 0)) {
            const me = acc.identities[g.id];
            if (!members.some((p) => p.id === me)) {
              throw new Error("Say who you are in this group before asking to delete it");
            }
            g.deletion_votes = [...new Set([...(g.deletion_votes ?? []), me])];
            if (!members.every((p) => g.deletion_votes?.includes(p.id))) return clone(g);
          }
          groups.splice(groups.indexOf(g), 1);
          delete acc.identities[g.id];
          acc.archived = acc.archived.filter((id) => id !== g.id);
          return null;
        }

        case "refuse_group_deletion": {
          const g = getGroups().find((x) => x.id === args?.groupId);
          if (!g) throw new Error("Group not found");
          g.deletion_votes = [];
          return clone(g);
        }

        case "set_group_archived": {
          const acc = requireAccount();
          if (!getGroups().some((x) => x.id === args?.groupId)) throw new Error("Group not found");
          acc.archived = acc.archived.filter((id) => id !== args.groupId);
          if (args.archived) acc.archived.push(args.groupId);
          return clone(acc);
        }

        case "update_group": {
          const g = getGroups().find((x) => x.id === args?.groupId);
          if (!g) throw new Error("Group not found");
          const name = String(args.name).trim();
          const currency = String(args.currency).trim().toUpperCase();
          if (!name) throw new Error("Group name cannot be empty");
          if (!/^[A-Z]{3}$/.test(currency)) {
            throw new Error("The currency must be a three-letter code, such as EUR");
          }
          const description = String(args.description ?? "").trim();
          if (description.length > 500) {
            throw new Error("This description is too long (500 characters at most)");
          }
          checkPicture(args.image);
          g.name = name;
          g.currency = currency;
          g.description = description;
          g.image = args.image ?? null;
          return clone(g);
        }

        case "rename_participant": {
          const g = getGroups().find((x) => x.id === args?.groupId);
          if (!g) throw new Error("Group not found");
          const p = g.participants.find((x) => x.id === args?.participantId);
          if (!p) throw new Error("Participant not found");
          const name = String(args.name).trim();
          if (!name) throw new Error("Participant name cannot be empty");
          // Payments still titled as recorded get the new name, like in doc.rs.
          const nameOf = (id: string, renamed: boolean) =>
            renamed && id === p.id ? name : g.participants.find((x) => x.id === id)?.name;
          for (const e of g.expenses) {
            const to = e.splits[0]?.participant_id;
            if (!e.is_reimbursement || e.splits.length !== 1 || ![e.paid_by, to].includes(p.id)) {
              continue;
            }
            const before = `Payment: ${nameOf(e.paid_by, false)} → ${nameOf(to, false)}`;
            const notes = e.title.startsWith(before) ? e.title.slice(before.length) : null;
            if (notes === null || (notes && !notes.startsWith(" ("))) continue;
            e.title = `Payment: ${nameOf(e.paid_by, true)} → ${nameOf(to, true)}${notes}`;
          }
          p.name = name;
          return clone(g);
        }

        case "add_participant": {
          const g = getGroups().find((x) => x.id === args?.groupId);
          if (!g) throw new Error("Group not found");
          g.participants.push({
            id: `p-${Date.now()}`,
            name: args.name,
            added_at: new Date().toISOString(),
            added_by: getAccount()?.identities[g.id] ?? null,
          });
          return clone(g);
        }

        case "add_expense": {
          args = expenseArgs(args);
          const g = getGroups().find((x) => x.id === args?.groupId);
          if (!g) throw new Error("Group not found");
          if (args.items.length > 0) {
            args.splits = itemsSplits(args.amountCents, args.original, args.items);
          }
          checkSplits(args.amountCents, args.original, args.splits);
          const who = paidBy(args);
          const now = new Date().toISOString();
          const createdAt = args?.createdAt || now;
          if (args.repeat && !["week", "month", "year"].includes(args.repeat)) {
            throw new Error("An expense repeats every week, month or year");
          }
          if (args.repeat && args.original) {
            throw new Error("A repeated expense has to be in the group's currency");
          }
          const recurring = args.repeat ? `rec-${Date.now()}` : null;
          const exp: MockExpense = {
            id: `exp-${Date.now()}`,
            group_id: args.groupId,
            title: args.title,
            category: args.category,
            amount_cents: args.amountCents,
            original: args.original ?? null,
            paid_by: who.paid_by,
            payers: who.payers,
            splits: args.splits,
            created_at: createdAt,
            updated_at: now,
            history: [],
            is_reimbursement: false,
            income: args.income,
            added_at: now,
            added_by: me(g),
            recurring,
            items: args.items,
            comments: [],
          };
          g.expenses.unshift(exp);
          if (recurring) {
            g.recurring ??= [];
            g.recurring.push({
              id: recurring,
              title: exp.title,
              category: exp.category,
              amount_cents: exp.amount_cents,
              income: exp.income,
              paid_by: exp.paid_by,
              payers: exp.payers,
              splits: exp.splits,
              every: args.repeat,
              start: createdAt,
              made: 1,
              added_by: me(g),
            });
            addDueExpenses(g);
          }
          return clone(g);
        }

        case "update_expense": {
          args = expenseArgs(args);
          const g = getGroups().find((x) => x.id === args?.groupId);
          if (!g) throw new Error("Group not found");
          const exp = g.expenses.find((x) => x.id === args?.expenseId);
          if (!exp) throw new Error("Expense not found");

          if (args.items.length > 0) {
            args.splits = itemsSplits(args.amountCents, args.original, args.items);
          }
          checkSplits(args.amountCents, args.original, args.splits);
          const who = paidBy(args);
          const prevTitle = exp.title;
          const prevAmount = exp.amount_cents;
          const prevPayer = exp.paid_by;
          const prevSplits = [...exp.splits];

          exp.history = exp.history || [];
          exp.history.push({
            edited_at: new Date().toISOString(),
            previous_title: prevTitle,
            previous_category: exp.category ?? null,
            previous_amount_cents: prevAmount,
            previous_paid_by: prevPayer,
            previous_payers: exp.payers ?? [],
            previous_splits: prevSplits,
            previous_original: exp.original ?? null,
            summary: `Amount changed to ${args.amountCents / 100} • Title updated to ${args.title}`,
            edited_by: me(g),
          });

          exp.title = args.title;
          exp.category = args.category;
          exp.amount_cents = args.amountCents;
          exp.original = args.original ?? null;
          exp.paid_by = who.paid_by;
          exp.payers = who.payers;
          exp.splits = args.splits;
          exp.items = args.items;
          if (args?.createdAt) {
            exp.created_at = args.createdAt;
          }
          exp.updated_at = new Date().toISOString();
          return clone(g);
        }

        case "delete_expense": {
          const g = getGroups().find((x) => x.id === args?.groupId);
          if (!g) throw new Error("Group not found");
          const exp = g.expenses.find((x) => x.id === args?.expenseId);
          if (!exp) throw new Error("Expense not found");
          g.expenses = g.expenses.filter((x) => x !== exp);
          g.trash ??= [];
          g.trash.unshift({
            expense: exp,
            deleted_at: new Date().toISOString(),
            deleted_by: me(g),
          });
          return clone(g);
        }

        case "restore_expense": {
          const g = getGroups().find((x) => x.id === args?.groupId);
          if (!g) throw new Error("Group not found");
          const deleted = g.trash?.find((d) => d.expense.id === args?.expenseId);
          if (!deleted) throw new Error("This expense is no longer in the trash");
          g.trash = g.trash?.filter((d) => d !== deleted);
          const exp = deleted.expense;
          const now = new Date().toISOString();
          exp.history = exp.history || [];
          exp.history.push({
            edited_at: now,
            previous_title: exp.title,
            previous_category: exp.category ?? null,
            previous_amount_cents: exp.amount_cents,
            previous_paid_by: exp.paid_by,
            previous_payers: exp.payers ?? [],
            previous_splits: [...exp.splits],
            previous_original: exp.original ?? null,
            summary: "Restored from the trash",
            edited_by: me(g),
          });
          exp.updated_at = now;
          g.expenses.unshift(exp);
          return clone(g);
        }

        case "purge_expense": {
          const g = getGroups().find((x) => x.id === args?.groupId);
          if (!g) throw new Error("Group not found");
          if (!g.trash?.some((d) => d.expense.id === args?.expenseId)) {
            throw new Error("This expense is no longer in the trash");
          }
          g.trash = g.trash.filter((d) => d.expense.id !== args.expenseId);
          return clone(g);
        }

        case "add_expense_comment": {
          const g = getGroups().find((x) => x.id === args?.groupId);
          if (!g) throw new Error("Group not found");
          const exp = g.expenses.find((x) => x.id === args?.expenseId);
          if (!exp) throw new Error("Expense not found");
          const text = String(args.text).trim();
          if (!text) throw new Error("A comment can't be empty");
          if (text.length > 500) {
            throw new Error("This comment is too long (500 characters at most)");
          }
          exp.comments ??= [];
          exp.comments.push({
            id: `com-${Date.now()}-${exp.comments.length}`,
            text,
            created_at: new Date().toISOString(),
            by: me(g),
          });
          return clone(g);
        }

        case "delete_expense_comment": {
          const g = getGroups().find((x) => x.id === args?.groupId);
          if (!g) throw new Error("Group not found");
          const exp = g.expenses.find((x) => x.comments?.some((c) => c.id === args?.commentId));
          if (!exp) throw new Error("This comment no longer exists");
          exp.comments = exp.comments?.filter((c) => c.id !== args.commentId);
          return clone(g);
        }

        case "stop_recurring_expense": {
          const g = getGroups().find((x) => x.id === args?.groupId);
          if (!g) throw new Error("Group not found");
          if (!g.recurring?.some((r) => r.id === args?.recurringId)) {
            throw new Error("This repeated expense no longer exists");
          }
          g.recurring = g.recurring.filter((r) => r.id !== args.recurringId);
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
            added_at: now,
            added_by: me(g),
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
