import { expect, test } from "@playwright/test";
import { installTauriMock, MOCK_PASSWORD } from "./fixtures/tauri-mock";

const now = new Date().toISOString();

const tripGroup = {
  id: "group-trip",
  name: "Lisbon Trip",
  currency: "EUR",
  created_at: now,
  participants: [
    { id: "p-alice", name: "Alice" },
    { id: "p-bob", name: "Bob" },
    { id: "p-carol", name: "Carol" },
  ],
  expenses: [
    {
      id: "exp-dinner",
      group_id: "group-trip",
      title: "Dinner",
      amount_cents: 9000,
      paid_by: "p-alice",
      splits: [
        { participant_id: "p-alice", shares: 1 },
        { participant_id: "p-bob", shares: 1 },
        { participant_id: "p-carol", shares: 1 },
      ],
      created_at: now,
      updated_at: now,
      history: [],
      is_reimbursement: false,
    },
  ],
};

test.beforeEach(async ({ page }) => {
  page.on("pageerror", (err) => console.error(">>> BROWSER ERROR:", err));
  await page.addInitScript(installTauriMock);
});

async function seed(page: import("@playwright/test").Page, data: Record<string, unknown>) {
  await page.addInitScript((d) => Object.assign(window, d), data);
}

test.describe("Removing members", () => {
  test("removed member leaves new-expense choices but keeps their open balance", async ({
    page,
  }) => {
    await seed(page, { __SEED_GROUPS__: [tripGroup] });
    await page.goto("/");
    await page.getByRole("button", { name: /Lisbon Trip/ }).click();

    await page.getByRole("button", { name: "Rename or remove Bob" }).click();
    await page.getByRole("dialog").getByRole("button", { name: "Remove from group" }).click();
    await page.getByRole("alertdialog").getByRole("button", { name: "Remove" }).click();
    await expect(page.getByRole("button", { name: "Rename or remove Bob" })).not.toBeVisible();
    await expect(page.getByText("2 participants")).toBeVisible();

    // Bob is no longer offered for new expenses...
    await page.getByRole("button", { name: "Add Expense", exact: true }).click();
    await page.locator("#select-expense-payer").click();
    await expect(page.getByRole("option", { name: "Alice (You)" })).toBeVisible();
    await expect(page.getByRole("option", { name: "Bob" })).toHaveCount(0);
    await page.keyboard.press("Escape");
    await page.getByRole("button", { name: "Cancel" }).click();

    // ...but still owes his share of dinner, and can settle it.
    await page.getByRole("tab", { name: "Balances" }).click();
    const bobCard = page.getByTestId("balance-card").filter({ hasText: "Bob" });
    await expect(bobCard.getByText("Removed")).toBeVisible();
    await expect(bobCard.getByText("-€30")).toBeVisible();
  });
});

test.describe("Confirmations", () => {
  test("deleting an expense asks first, and cancel keeps it", async ({ page }) => {
    await seed(page, { __SEED_GROUPS__: [tripGroup] });
    await page.goto("/");
    await page.getByRole("button", { name: /Lisbon Trip/ }).click();

    const openDelete = async () => {
      await page.getByRole("button", { name: "Actions for Dinner" }).click();
      await page.getByRole("menuitem", { name: "Delete" }).click();
    };

    await openDelete();
    const confirmDialog = page.getByRole("alertdialog");
    await expect(confirmDialog).toContainText('Delete "Dinner"?');
    await confirmDialog.getByRole("button", { name: "Cancel" }).click();
    await expect(confirmDialog).not.toBeVisible();
    await expect(page.getByRole("heading", { name: "Dinner", exact: true })).toBeVisible();

    await openDelete();
    await confirmDialog.getByRole("button", { name: "Delete" }).click();
    await expect(page.getByText("No expenses recorded yet")).toBeVisible();
  });
});

test.describe("Sharing and joining", () => {
  test("every group has an invite link and QR code", async ({ page }) => {
    await seed(page, { __SEED_GROUPS__: [tripGroup] });
    await page.goto("/");
    await page.getByRole("button", { name: /Lisbon Trip/ }).click();

    await page.getByRole("button", { name: "Invite", exact: true }).click();
    await expect(page.locator("#share-invite-code")).toHaveValue(
      /^http:\/\/localhost:8787\/join#v=2&g=group-trip&k=/
    );
    await expect(
      page.getByRole("img", { name: 'QR code of the invite to "Lisbon Trip"' })
    ).toBeVisible();
    await expect(page.getByRole("button", { name: "Copy Link" })).toBeVisible();
    await page.getByRole("button", { name: "Close" }).click();
  });

  test("shares the invite through the phone's share sheet", async ({ page }) => {
    await seed(page, { __SEED_GROUPS__: [tripGroup], __NATIVE__: { share: true } });
    await page.goto("/");
    await page.getByRole("button", { name: /Lisbon Trip/ }).click();
    await page.getByRole("button", { name: "Invite", exact: true }).click();

    await page.getByRole("button", { name: "Share", exact: true }).click();
    await expect
      .poll(() => page.evaluate(() => (window as any).__shared))
      .toEqual([
        'Join "Lisbon Trip" on ezcount: http://localhost:8787/join#v=2&g=group-trip&k=mock-key',
      ]);
  });

  test("opening an invite link fills in the join dialog", async ({ page }) => {
    await seed(page, {
      __REMOTE_GROUPS__: [tripGroup],
      __OPENED_WITH__: "https://sync.example.com/join#v=2&g=group-trip&k=k",
    });
    await page.goto("/");

    const join = page.getByRole("dialog", { name: "Join a Group" });
    await expect(join.getByLabel("Invite link")).toHaveValue(
      "https://sync.example.com/join#v=2&g=group-trip&k=k"
    );
    await join.getByRole("button", { name: "Join Group" }).click();
    await expect(page.getByRole("heading", { name: "Lisbon Trip" })).toBeVisible();
  });

  test("an invite link opened while logged out waits for the login", async ({ page }) => {
    await seed(page, {
      __LOGGED_OUT__: true,
      __OPENED_WITH__:
        "ezcount://join?server=https%3A%2F%2Fsync.example.com&group=group-trip&key=k&v=2",
    });
    await page.goto("/");
    await page.getByLabel("Username").fill("alice");
    await page.getByLabel("Password").fill(MOCK_PASSWORD);
    await page.getByRole("button", { name: "Log In" }).click();

    const join = page.getByRole("dialog", { name: "Join a Group" });
    await expect(join.getByLabel("Invite link")).toHaveValue(/^ezcount:\/\/join\?server=/);
  });

  test("scanning an invite QR code joins the group", async ({ page }) => {
    await seed(page, {
      __REMOTE_GROUPS__: [tripGroup],
      __NATIVE__: { scan: true },
      __SCANNED__: "https://sync.example.com/join#v=2&g=group-trip&k=k",
    });
    await page.goto("/");

    await page.getByRole("button", { name: "Join with Code" }).click();
    await page.getByRole("button", { name: "Scan QR Code" }).click();
    await expect(page.getByRole("heading", { name: "Lisbon Trip" })).toBeVisible();
    await expect(page.locator("html")).not.toHaveClass(/scanning/);
  });

  test("scanning is offered only where the device can", async ({ page }) => {
    await page.goto("/");
    await page.getByRole("button", { name: "Join with Code" }).click();
    await expect(page.getByRole("dialog", { name: "Join a Group" })).toBeVisible();
    await expect(page.getByRole("button", { name: "Scan QR Code" })).toHaveCount(0);
  });

  test("joins a group from an invite code", async ({ page }) => {
    await seed(page, { __REMOTE_GROUPS__: [tripGroup] });
    await page.goto("/");

    await page.getByRole("button", { name: "Join with Code" }).click();
    await page.locator("#input-invite-code").fill("not a code");
    await page.getByRole("button", { name: "Join Group" }).click();
    await expect(page.getByRole("alert")).toContainText("not a valid");

    await page
      .locator("#input-invite-code")
      .fill("https://sync.example.com/join#v=2&g=group-trip&k=k");
    await page.getByRole("button", { name: "Join Group" }).click();
    await expect(page.getByRole("heading", { name: "Lisbon Trip" })).toBeVisible();

    // Joining asks who you are, once.
    const who = page.getByRole("dialog", { name: 'Who are you in "Lisbon Trip"?' });
    await who.getByRole("button", { name: "Bob", exact: true }).click();
    await expect(who).not.toBeVisible();
    await expect(page.getByText("Your balance", { exact: true })).toBeVisible();
    await expect(page.getByText("-€30")).toBeVisible();
  });

  test("adds yourself when you're not in the list", async ({ page }) => {
    await seed(page, { __SEED_GROUPS__: [tripGroup], __SEED_IDENTITIES__: {} });
    await page.goto("/");
    await page.getByRole("button", { name: /Lisbon Trip/ }).click();

    const who = page.getByRole("dialog", { name: 'Who are you in "Lisbon Trip"?' });
    await who.getByRole("button", { name: "I'm not in the list" }).click();
    await expect(who.getByLabel("Your name in this group")).toHaveValue("alice");
    await who.getByLabel("Your name in this group").fill("Dana");
    await who.getByRole("button", { name: "Add Me" }).click();
    await expect(page.getByText("Your balance", { exact: true })).toBeVisible();
    await expect(page.getByText("4 participants")).toBeVisible();
  });

  test("closing the question leaves a button to answer later", async ({ page }) => {
    await seed(page, { __SEED_GROUPS__: [tripGroup], __SEED_IDENTITIES__: {} });
    await page.goto("/");
    await page.getByRole("button", { name: /Lisbon Trip/ }).click();

    await page.getByRole("dialog").getByRole("button", { name: "Close" }).click();
    await page.getByRole("button", { name: "Who are you in this group?" }).click();
    await page.getByRole("dialog").getByRole("button", { name: "Carol", exact: true }).click();
    await expect(page.getByText("Your balance", { exact: true })).toBeVisible();
  });

  test("offers to show what another device changed in the open group", async ({ page }) => {
    await seed(page, { __SEED_GROUPS__: [tripGroup] });
    await page.goto("/");
    await page.getByRole("button", { name: /Lisbon Trip/ }).click();
    await expect(page.getByRole("heading", { name: "Dinner" })).toBeVisible();

    // Simulate a background sync that imported a new expense.
    await page.evaluate(async () => {
      const w = window as any;
      await w.__TAURI_INTERNALS__.invoke("add_expense", {
        groupId: "group-trip",
        title: "Museum",
        amountCents: 3000,
        paidBy: "p-bob",
        splits: [{ participant_id: "p-bob", shares: 1 }],
        createdAt: null,
      });
      w.__emitMockEvent("sync-updated", { group_id: "group-trip", changed: true });
    });
    // Nothing moves until the user asks.
    const refresh = page.getByRole("button", { name: /New changes from the group/ });
    await expect(refresh).toBeVisible();
    await expect(page.getByRole("heading", { name: "Museum" })).toHaveCount(0);
    await refresh.click();
    await expect(page.getByRole("heading", { name: "Museum" })).toBeVisible();
    await expect(refresh).toHaveCount(0);
  });
});

/** The recovery key dialog: it shows the key and stays up until the user says they saved it. */
async function saveRecoveryKey(page: import("@playwright/test").Page, key: RegExp) {
  const dialog = page.getByRole("dialog", { name: "Save your recovery key" });
  await expect(dialog.getByLabel("Recovery key")).toHaveText(key);
  const done = dialog.getByRole("button", { name: "Done" });
  await expect(done).toBeDisabled();
  await page.keyboard.press("Escape");
  await expect(dialog).toBeVisible();
  await dialog.getByLabel("I've saved it somewhere safe").check();
  await done.click();
  await expect(dialog).not.toBeVisible();
}

test.describe("Account", () => {
  test("signs up, then logs out", async ({ page }) => {
    await seed(page, { __LOGGED_OUT__: true, __TAKEN_USERNAMES__: ["bob"] });
    await page.goto("/");
    await expect(page.getByRole("heading", { name: "Welcome back" })).toBeVisible();

    await page.getByRole("tab", { name: "Sign up" }).click();
    await page.getByLabel("Username").fill("bob");
    await page.getByLabel("Password", { exact: true }).fill("tangerine kayak mosaic");
    await page.getByLabel("Confirm password").fill("different");
    await page.getByRole("button", { name: "Create Account" }).click();
    await expect(page.getByText("The passwords don't match.")).toBeVisible();

    await page.getByLabel("Confirm password").fill("tangerine kayak mosaic");
    await page.getByRole("button", { name: "Create Account" }).click();
    await expect(page.getByText('The username "bob" is already taken')).toBeVisible();

    await page.getByLabel("Username").fill("Robert");
    await page.getByRole("button", { name: "Create Account" }).click();
    await saveRecoveryKey(page, /^MOCK-KEY1-/);
    await expect(page.getByText("No groups yet")).toBeVisible();

    await page.getByRole("button", { name: "Menu", exact: true }).click();
    await expect(page.getByRole("menu")).toContainText("robert");
    await page.getByRole("menuitem", { name: /Account/ }).click();
    await page
      .getByRole("dialog", { name: "Account" })
      .getByRole("button", { name: "Log out" })
      .click();
    await page.getByRole("alertdialog").getByRole("button", { name: "Log Out" }).click();
    await expect(page.getByRole("heading", { name: "Welcome back" })).toBeVisible();
  });

  test("rates the new password and refuses weak ones", async ({ page }) => {
    await seed(page, { __LOGGED_OUT__: true });
    await page.goto("/");
    await page.getByRole("tab", { name: "Sign up" }).click();
    await page.getByLabel("Username").fill("carol");
    const password = page.getByLabel("Password", { exact: true });
    const create = page.getByRole("button", { name: "Create Account" });

    await password.fill("password");
    await expect(password).toHaveAccessibleDescription(
      "Too weak · This is a top-10 common password."
    );
    await expect(create).toBeDisabled();

    await password.fill("carol-rocks-99");
    await expect(password).toHaveAccessibleDescription(/^Too weak · /);
    await expect(create).toBeDisabled();

    await password.fill("tangerine kayak mosaic");
    await expect(password).toHaveAccessibleDescription("Strong");
    await page.getByLabel("Confirm password").fill("tangerine kayak mosaic");
    await create.click();
    await saveRecoveryKey(page, /^MOCK-KEY1-/);
    await expect(page.getByText("No groups yet")).toBeVisible();
  });

  test("warns when the server can't store a recovery key", async ({ page }) => {
    await seed(page, { __LOGGED_OUT__: true, __OLD_RELAY__: true });
    await page.goto("/");
    await page.getByRole("tab", { name: "Sign up" }).click();
    await page.getByLabel("Username").fill("carol");
    await page.getByLabel("Password", { exact: true }).fill("tangerine kayak mosaic");
    await page.getByLabel("Confirm password").fill("tangerine kayak mosaic");
    await page.getByRole("button", { name: "Create Account" }).click();

    await expect(page.getByText("No groups yet")).toBeVisible();
    await expect(page.getByText("Your account has no recovery key")).toBeVisible();
    await expect(page.getByRole("dialog", { name: "Save your recovery key" })).toHaveCount(0);
  });

  test("resets a forgotten password with the recovery key", async ({ page }) => {
    const key = "7KQ2-M9XD-AAAA-BBBB-CCCC-DDDD-EEEE-FFFF";
    await seed(page, { __LOGGED_OUT__: true, __RECOVERY_KEY__: key });
    await page.goto("/");
    await page.getByRole("button", { name: "Forgot password?" }).click();
    await expect(page.getByRole("heading", { name: "Reset your password" })).toBeVisible();

    await page.getByLabel("Username").fill("alice");
    await page.getByLabel("Recovery key").fill("7KQ2-M9XD-0000-0000-0000-0000-0000-0000");
    await page.getByLabel("New password", { exact: true }).fill("juniper walrus lantern");
    await page.getByLabel("Confirm new password").fill("juniper walrus lantern");
    await page.getByRole("button", { name: "Reset Password" }).click();
    await expect(page.getByText("Wrong username or recovery key")).toBeVisible();

    // Typed as people do: lowercase, spaces instead of dashes.
    await page.getByLabel("Recovery key").fill(key.toLowerCase().replaceAll("-", " "));
    await page.getByRole("button", { name: "Reset Password" }).click();
    await expect(page.getByText("The recovery key you used no longer works")).toBeVisible();
    await saveRecoveryKey(page, /^MOCK-KEY1-/);

    // The new password is the one that works now.
    await page.getByRole("button", { name: "Menu", exact: true }).click();
    await page.getByRole("menuitem", { name: /Account/ }).click();
    await page
      .getByRole("dialog", { name: "Account" })
      .getByRole("button", { name: "Log out" })
      .click();
    await page.getByRole("alertdialog").getByRole("button", { name: "Log Out" }).click();
    await page.getByLabel("Username").fill("alice");
    await page.getByLabel("Password").fill(MOCK_PASSWORD);
    await page.getByRole("button", { name: "Log In" }).click();
    await expect(page.getByText("Wrong username or password")).toBeVisible();
    await page.getByLabel("Password").fill("juniper walrus lantern");
    await page.getByRole("button", { name: "Log In" }).click();
    await expect(page.getByText("No groups yet")).toBeVisible();
  });

  test("shows a code that logs another device in, for a while", async ({ page }) => {
    await seed(page, { __LINK_SECONDS__: 2 });
    await page.goto("/");
    await page.getByRole("button", { name: "Menu", exact: true }).click();
    await page.getByRole("menuitem", { name: "Connect a device" }).click();
    const dialog = page.getByRole("dialog", { name: "Connect another device" });

    await dialog.getByLabel("Password").fill("not my password");
    await dialog.getByRole("button", { name: "Show Code" }).click();
    await expect(dialog.getByText("Wrong password")).toBeVisible();

    await dialog.getByLabel("Password").fill(MOCK_PASSWORD);
    await dialog.getByRole("button", { name: "Show Code" }).click();
    await expect(
      dialog.getByRole("img", { name: /QR code that logs another device/ })
    ).toBeVisible();
    await expect(dialog.getByText(/Works once, for another 0:0\d/)).toBeVisible();

    // Past its time, the code is gone and a new one asks for the password again.
    await expect(dialog.getByText("This code has expired.")).toBeVisible();
    await expect(dialog.getByRole("img")).not.toBeVisible();
    await expect(dialog.getByLabel("Password")).toHaveValue("");
    await expect(dialog.getByRole("button", { name: "New Code" })).toBeVisible();
  });

  test("logs in by scanning the code another device shows", async ({ page }) => {
    await seed(page, {
      __LOGGED_OUT__: true,
      __NATIVE__: { scan: true },
      __SCANNED__: "ezcount://login?server=https%3A%2F%2Fsync.example.com&code=mock-code",
    });
    await page.goto("/");

    await page.getByRole("button", { name: "Scan a code to log in" }).click();
    await expect(page.getByText("No groups yet")).toBeVisible();
    await expect(page.locator("html")).not.toHaveClass(/scanning/);
    await page.getByRole("button", { name: "Menu", exact: true }).click();
    await page.getByRole("menuitem", { name: /Account/ }).click();
    await expect(page.getByRole("dialog", { name: "Account" })).toContainText("sync.example.com");
  });

  test("a scanned code that no longer works says so", async ({ page }) => {
    await seed(page, {
      __LOGGED_OUT__: true,
      __NATIVE__: { scan: true },
      __SCANNED__: "ezcount://login?server=https%3A%2F%2Fsync.example.com&code=used",
    });
    await page.goto("/");

    await page.getByRole("button", { name: "Scan a code to log in" }).click();
    await expect(page.getByText(/This code has expired or was already used/)).toBeVisible();
    await expect(page.getByRole("button", { name: "Log In", exact: true })).toBeVisible();
  });

  test("scanning to log in is offered only where the device can scan", async ({ page }) => {
    await seed(page, { __LOGGED_OUT__: true });
    await page.goto("/");
    await expect(page.getByRole("button", { name: "Log In" })).toBeVisible();
    await expect(page.getByRole("button", { name: "Scan a code to log in" })).toHaveCount(0);
  });

  test("changes the password from the account menu", async ({ page }) => {
    await page.goto("/");
    await page.getByRole("button", { name: "Menu", exact: true }).click();
    await page.getByRole("menuitem", { name: /Account/ }).click();
    await page
      .getByRole("dialog", { name: "Account" })
      .getByRole("button", { name: "Change password" })
      .click();
    const dialog = page.getByRole("dialog", { name: "Change password" });

    await dialog.getByLabel("Current password").fill("not my password");
    await dialog.getByLabel("New password", { exact: true }).fill("juniper walrus lantern");
    await dialog.getByLabel("Confirm new password").fill("juniper walrus lantern");
    await dialog.getByRole("button", { name: "Change Password" }).click();
    await expect(dialog.getByText("Your current password is wrong")).toBeVisible();

    await dialog.getByLabel("Current password").fill(MOCK_PASSWORD);
    await dialog.getByRole("button", { name: "Change Password" }).click();
    await expect(dialog).not.toBeVisible();
    await expect(page.getByText("Password changed")).toBeVisible();
  });

  test("makes a new recovery key from the account menu", async ({ page }) => {
    await page.goto("/");
    await page.getByRole("button", { name: "Menu", exact: true }).click();
    await page.getByRole("menuitem", { name: /Account/ }).click();
    await page
      .getByRole("dialog", { name: "Account" })
      .getByRole("button", { name: "New recovery key" })
      .click();
    const dialog = page.getByRole("dialog", { name: "New recovery key" });

    await dialog.getByLabel("Password").fill("not my password");
    await dialog.getByRole("button", { name: "Create New Key" }).click();
    await expect(dialog.getByText("Wrong password")).toBeVisible();

    await dialog.getByLabel("Password").fill(MOCK_PASSWORD);
    await dialog.getByRole("button", { name: "Create New Key" }).click();
    await expect(page.getByText("Your previous recovery key no longer works")).toBeVisible();
    await saveRecoveryKey(page, /^MOCK-KEY1-/);
  });

  test("logging in doesn't rate the password", async ({ page }) => {
    await seed(page, { __LOGGED_OUT__: true });
    await page.goto("/");
    await page.getByLabel("Password").fill("x");
    await expect(page.getByText("Too weak")).toHaveCount(0);
    await expect(page.getByRole("button", { name: "Log In" })).toBeEnabled();
  });

  test("the server is built in, and can be changed", async ({ page }) => {
    await seed(page, { __LOGGED_OUT__: true });
    await page.goto("/");
    await expect(page.getByLabel("Server", { exact: true })).toHaveCount(0);
    await expect(page.getByText("Server: localhost:8787")).toBeVisible();

    // A relay of your own.
    await page.getByRole("button", { name: "Change server" }).click();
    await expect(page.getByLabel("Server", { exact: true })).toHaveValue("http://localhost:8787");
    await page.getByLabel("Server", { exact: true }).fill("https://relay.example.com");
    await page.getByLabel("Username").fill("alice");
    await page.getByLabel("Password").fill(MOCK_PASSWORD);
    await page.getByRole("button", { name: "Log In" }).click();
    await page.getByRole("button", { name: "Menu", exact: true }).click();
    await page.getByRole("menuitem", { name: /Account/ }).click();
    await expect(page.getByRole("dialog", { name: "Account" })).toContainText("relay.example.com");

    // It's remembered for the next login, with a way back to the default.
    await page
      .getByRole("dialog", { name: "Account" })
      .getByRole("button", { name: "Log out" })
      .click();
    await page.getByRole("alertdialog").getByRole("button", { name: "Log Out" }).click();
    await expect(page.getByText("Server: relay.example.com")).toBeVisible();
    await page.getByRole("button", { name: "Change server" }).click();
    await page.getByRole("button", { name: "Use the default server" }).click();
    await expect(page.getByLabel("Server", { exact: true })).toHaveValue("http://localhost:8787");
  });

  test("logs in and rejects a wrong password", async ({ page }) => {
    await seed(page, { __LOGGED_OUT__: true });
    await page.goto("/");
    await page.getByLabel("Username").fill("alice");
    await page.getByLabel("Password").fill("wrong password");
    await page.getByRole("button", { name: "Log In" }).click();
    await expect(page.getByText("Wrong username or password")).toBeVisible();

    await page.getByLabel("Password").fill(MOCK_PASSWORD);
    await page.getByRole("button", { name: "Log In" }).click();
    // Groups arrive in the background after logging in.
    await expect(page.getByText("No groups yet")).toBeVisible();
  });

  test("warns before logging out with changes not uploaded", async ({ page }) => {
    await seed(page, { __SEED_GROUPS__: [tripGroup], __UNSYNCED__: true });
    await page.goto("/");
    await page.getByRole("button", { name: "Menu", exact: true }).click();
    await page.getByRole("menuitem", { name: /Account/ }).click();
    await page
      .getByRole("dialog", { name: "Account" })
      .getByRole("button", { name: "Log out" })
      .click();
    await page.getByRole("alertdialog").getByRole("button", { name: "Log Out" }).click();

    const warning = page.getByRole("alertdialog");
    await expect(warning).toContainText("not uploaded yet");
    await warning.getByRole("button", { name: "Cancel" }).click();
    await expect(page.getByRole("heading", { name: "Your Groups" })).toBeVisible();
  });

  test("leaves the open group when another device removed it", async ({ page }) => {
    await seed(page, { __SEED_GROUPS__: [tripGroup] });
    await page.goto("/");
    await page.getByRole("button", { name: /Lisbon Trip/ }).click();
    await expect(page.getByRole("heading", { name: "Lisbon Trip" })).toBeVisible();

    await page.evaluate(() => {
      const w = window as any;
      w.__removeGroupElsewhere("group-trip");
      w.__emitMockEvent("account-updated", null);
    });
    await expect(page.getByText("No groups yet")).toBeVisible();
  });
});

test("shows a warning when saved data could not be loaded", async ({ page }) => {
  await seed(page, { __STORAGE_WARNINGS__: ["Group g1 could not be loaded (bad data)."] });
  await page.goto("/");
  await expect(page.getByText("Some saved data could not be loaded")).toBeVisible();
  await expect(page.getByText("Group g1 could not be loaded")).toBeVisible();
  await page.getByRole("button", { name: "Dismiss" }).click();
  await expect(page.getByText("Some saved data could not be loaded")).not.toBeVisible();
});
