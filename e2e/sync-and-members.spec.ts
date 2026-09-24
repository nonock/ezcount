import { expect, test } from "@playwright/test";
import { installTauriMock } from "./fixtures/tauri-mock";

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
  page.on("dialog", (dialog) => dialog.accept());
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
    await expect(page.getByRole("button", { name: "Remove Bob" })).not.toBeVisible();
    await expect(page.getByText("2 participants")).toBeVisible();

    // Bob is no longer offered for new expenses...
    await page.getByRole("button", { name: "Add Expense", exact: true }).click();
    const payer = page.locator("#select-expense-payer");
    await expect(payer.locator("option", { hasText: "Bob" })).toHaveCount(0);
    await page.getByRole("button", { name: "Cancel" }).click();

    // ...but still owes his share of dinner, and can settle it.
    await page.getByRole("button", { name: /Balances/ }).click();
    const bobCard = page.locator("div.rounded-xl", { hasText: "Bob" }).filter({
      hasText: "Removed",
    });
    await expect(bobCard.getByText("-30.00 €")).toBeVisible();
  });
});

test.describe("Sharing and joining", () => {
  test("shares a group and shows its invite code", async ({ page }) => {
    await seed(page, { __SEED_GROUPS__: [tripGroup] });
    await page.goto("/");
    await page.getByRole("button", { name: /Lisbon Trip/ }).click();

    await page.getByRole("button", { name: "Share", exact: true }).click();
    await page.locator("#input-sync-server").fill("ftp://nope");
    await page.getByRole("button", { name: "Start Syncing" }).click();
    await expect(page.getByRole("alert")).toContainText("http://");

    await page.locator("#input-sync-server").fill("https://sync.example.com/");
    await page.getByRole("button", { name: "Start Syncing" }).click();
    await expect(page.locator("#share-invite-code")).toHaveValue(
      /^ezcount:\/\/join\?server=https%3A%2F%2Fsync\.example\.com&group=group-trip&key=/
    );
    await page.getByRole("button", { name: "Dismiss dialog" }).click();
    await expect(page.getByRole("button", { name: "Shared" })).toBeVisible();
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
    await expect(page.getByRole("button", { name: "Shared" })).toBeVisible();
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

test("shows a warning when saved data could not be loaded", async ({ page }) => {
  await seed(page, { __STORAGE_WARNINGS__: ["Group g1 could not be loaded (bad data)."] });
  await page.goto("/");
  await expect(page.getByText("Some saved data could not be loaded")).toBeVisible();
  await expect(page.getByText("Group g1 could not be loaded")).toBeVisible();
  await page.getByRole("button", { name: "Dismiss" }).click();
  await expect(page.getByText("Some saved data could not be loaded")).not.toBeVisible();
});
