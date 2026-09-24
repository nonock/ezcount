import { expect, test } from "@playwright/test";
import { MOCK_PASSWORD, installTauriMock } from "./fixtures/tauri-mock";

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

    await page.getByRole("button", { name: "Remove Bob" }).click();
    await page.getByRole("alertdialog").getByRole("button", { name: "Remove" }).click();
    await expect(page.getByRole("button", { name: "Remove Bob" })).not.toBeVisible();
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
    await expect(bobCard.getByText("-30.00 €")).toBeVisible();
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
    await expect(page.getByRole("heading", { name: "Dinner" })).toBeVisible();

    await openDelete();
    await confirmDialog.getByRole("button", { name: "Delete" }).click();
    await expect(page.getByText("No expenses recorded yet")).toBeVisible();
  });
});

test.describe("Sharing and joining", () => {
  test("every group has an invite code", async ({ page }) => {
    await seed(page, { __SEED_GROUPS__: [tripGroup] });
    await page.goto("/");
    await page.getByRole("button", { name: /Lisbon Trip/ }).click();

    await page.getByRole("button", { name: "Invite", exact: true }).click();
    await expect(page.locator("#share-invite-code")).toHaveValue(
      /^ezcount:\/\/join\?server=http%3A%2F%2Flocalhost%3A8787&group=group-trip&key=/
    );
    await page.getByRole("button", { name: "Close" }).click();
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
      .fill("ezcount://join?server=https%3A%2F%2Fsync.example.com&group=group-trip&key=k");
    await page.getByRole("button", { name: "Join Group" }).click();
    await expect(page.getByRole("heading", { name: "Lisbon Trip" })).toBeVisible();

    // Joining asks who you are, once.
    const who = page.getByRole("dialog", { name: 'Who are you in "Lisbon Trip"?' });
    await who.getByRole("button", { name: "Bob", exact: true }).click();
    await expect(who).not.toBeVisible();
    await expect(page.getByText("You're Bob")).toBeVisible();
    await expect(page.getByText("-30.00 €")).toBeVisible();
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
    await expect(page.getByText("You're Dana")).toBeVisible();
    await expect(page.getByText("4 participants")).toBeVisible();
  });

  test("closing the question leaves a button to answer later", async ({ page }) => {
    await seed(page, { __SEED_GROUPS__: [tripGroup], __SEED_IDENTITIES__: {} });
    await page.goto("/");
    await page.getByRole("button", { name: /Lisbon Trip/ }).click();

    await page.getByRole("dialog").getByRole("button", { name: "Close" }).click();
    await page.getByRole("button", { name: "Who are you in this group?" }).click();
    await page.getByRole("dialog").getByRole("button", { name: "Carol", exact: true }).click();
    await expect(page.getByText("You're Carol")).toBeVisible();
  });

  test("reloads the open group when another device's changes arrive", async ({ page }) => {
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
    await expect(page.getByRole("heading", { name: "Museum" })).toBeVisible();
  });
});

test.describe("Account", () => {
  test("signs up, then logs out", async ({ page }) => {
    await seed(page, { __LOGGED_OUT__: true, __TAKEN_USERNAMES__: ["bob"] });
    await page.goto("/");
    await expect(page.getByRole("heading", { name: "Welcome back" })).toBeVisible();

    await page.getByRole("tab", { name: "Sign up" }).click();
    await page.getByLabel("Username").fill("bob");
    await page.getByLabel("Password", { exact: true }).fill("long enough");
    await page.getByLabel("Confirm password").fill("different");
    await page.getByRole("button", { name: "Create Account" }).click();
    await expect(page.getByText("The passwords don't match.")).toBeVisible();

    await page.getByLabel("Confirm password").fill("long enough");
    await page.getByRole("button", { name: "Create Account" }).click();
    await expect(page.getByText('The username "bob" is already taken')).toBeVisible();

    await page.getByLabel("Username").fill("Robert");
    await page.getByRole("button", { name: "Create Account" }).click();
    await expect(page.getByText("No groups yet")).toBeVisible();

    await page.getByRole("button", { name: "Account" }).click();
    await expect(page.getByRole("menu")).toContainText("robert");
    await page.getByRole("menuitem", { name: "Log out" }).click();
    await page.getByRole("alertdialog").getByRole("button", { name: "Log Out" }).click();
    await expect(page.getByRole("heading", { name: "Welcome back" })).toBeVisible();
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
    await page.getByRole("button", { name: "Account" }).click();
    await page.getByRole("menuitem", { name: "Log out" }).click();
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
