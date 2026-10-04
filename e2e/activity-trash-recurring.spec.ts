import { expect, type Page, test } from "@playwright/test";
import { installTauriMock } from "./fixtures/tauri-mock";

const created = "2026-01-05T12:00:00.000Z";
const everyone = ["p-alice", "p-bob"].map((id) => ({ participant_id: id, shares: 1 }));

const flat = {
  id: "group-flat",
  name: "Flat",
  currency: "EUR",
  created_at: created,
  participants: [
    { id: "p-alice", name: "Alice" },
    { id: "p-bob", name: "Bob" },
  ],
  expenses: [
    {
      id: "e-dinner",
      group_id: "group-flat",
      title: "Dinner",
      amount_cents: 6000,
      paid_by: "p-alice",
      splits: everyone,
      created_at: created,
      updated_at: created,
      history: [],
      is_reimbursement: false,
    },
  ],
};

async function open(page: Page, group: object = flat) {
  page.on("pageerror", (err) => console.error(">>> BROWSER ERROR:", err));
  await page.addInitScript(installTauriMock);
  await page.addInitScript((d) => Object.assign(window, d), { __SEED_GROUPS__: [group] });
  await page.goto("/");
  await page.getByRole("button", { name: /Flat/ }).click();
}

const titles = (page: Page) => page.getByTestId("expense-item").getByRole("heading");

async function openFromMenu(page: Page, item: string) {
  await page.getByRole("button", { name: "Activity and trash" }).click();
  await page.getByRole("menuitem", { name: item }).click();
}

test("money that came in is owed by who received it", async ({ page }) => {
  await open(page, { ...flat, expenses: [] });
  await page.getByRole("button", { name: "Add Expense" }).click();
  const dialog = page.getByRole("dialog");
  await dialog.getByRole("tab", { name: "Income" }).click();
  await expect(dialog.getByRole("heading", { name: "Add Income" })).toBeVisible();
  await dialog.getByLabel("Description").fill("Deposit back");
  await dialog.getByLabel("Amount", { exact: true }).fill("100");
  await expect(dialog.getByLabel("Received by")).toContainText("Alice");
  await dialog.getByRole("button", { name: "Save Income" }).click();
  await expect(dialog).not.toBeVisible();

  const item = page.getByTestId("expense-item");
  await expect(item).toContainText("Deposit back");
  await expect(item).toContainText("Income");
  await expect(item).toContainText("Received by Alice");

  // Alice holds 100.00, half of which is Bob's.
  await page.getByRole("tab", { name: "Balances" }).click();
  const card = (name: string) => page.getByTestId("balance-card").filter({ hasText: name });
  await expect(card("Bob")).toContainText("+€50");
  await expect(card("Alice")).toContainText("-€50");

  // It isn't spending.
  await page.getByRole("tab", { name: /Stats/ }).click();
  await expect(page.getByText("Nothing to count yet")).toBeVisible();
});

test("a transfer between two people is recorded as a payment", async ({ page }) => {
  await open(page);
  await page.getByRole("button", { name: "Add Expense" }).click();
  const tabs = page.getByRole("dialog").getByRole("tablist");
  await expect(tabs).toBeVisible();
  // Once the dialog has finished appearing.
  await page.waitForTimeout(300);
  const before = await page.getByRole("dialog").boundingBox();
  const tabsBefore = await tabs.boundingBox();
  await tabs.getByRole("tab", { name: "Transfer" }).click();
  const dialog = page.getByRole("dialog", { name: "Record Reimbursement" });
  // The same dialog, shorter from the bottom: the other kinds stay a tab away, where they were.
  await expect(dialog.getByLabel("From (sender)")).toBeVisible();
  const after = await dialog.boundingBox();
  expect(after?.y).toBe(before?.y);
  expect(after?.height).toBeLessThan(before?.height ?? 0);
  expect(await tabs.boundingBox()).toEqual(tabsBefore);
  await tabs.getByRole("tab", { name: "Income" }).click();
  await expect(page.getByRole("dialog", { name: "Add Income" })).toBeVisible();
  expect(await tabs.boundingBox()).toEqual(tabsBefore);
  await tabs.getByRole("tab", { name: "Transfer" }).click();
  await dialog.getByLabel("Amount", { exact: true }).fill("20");
  await dialog.getByRole("button", { name: "Confirm Payment" }).click();
  await expect(dialog).not.toBeVisible();
  await expect(titles(page)).toHaveText(["Payment: Alice → Bob", "Dinner"]);
});

test("a deleted expense waits in the trash", async ({ page }) => {
  await open(page);
  const remove = async () => {
    await page.getByRole("button", { name: "Actions for Dinner" }).click();
    await page.getByRole("menuitem", { name: "Delete" }).click();
  };

  // Deleting doesn't ask: it is undone from the message.
  await remove();
  await expect(page.getByText("No expenses recorded yet")).toBeVisible();
  await expect(page.getByText('"Dinner" moved to the trash')).toBeVisible();
  await page.getByRole("button", { name: "Undo" }).click();
  await expect(titles(page)).toHaveText(["Dinner"]);

  // Or from the trash, later.
  await remove();
  await expect(page.getByText("No expenses recorded yet")).toBeVisible();
  await openFromMenu(page, "Trash");
  const trash = page.getByRole("dialog", { name: "Trash" });
  await expect(trash.getByRole("listitem")).toContainText(["Dinner"]);
  await expect(trash.getByRole("listitem")).toContainText(["Deleted by Alice"]);
  await trash.getByRole("button", { name: "Restore Dinner" }).click();
  await expect(trash.getByText("The trash is empty")).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(titles(page)).toHaveText(["Dinner"]);
  // Its history says it came back.
  await expect(page.getByRole("button", { name: /Edited \(2\)/ })).toBeVisible();

  // Removing it from the trash is for good, and asks.
  await remove();
  await openFromMenu(page, "Trash");
  await trash.getByRole("button", { name: "Delete Dinner for good" }).click();
  await page.getByRole("alertdialog").getByRole("button", { name: "Delete for good" }).click();
  await expect(trash.getByText("The trash is empty")).toBeVisible();
});

test("a repeated expense comes back by itself, until it is stopped", async ({ page }) => {
  // Rent since the first of the month two months ago: three are due.
  const start = new Date();
  start.setUTCDate(1);
  start.setUTCMonth(start.getUTCMonth() - 2);
  start.setUTCHours(0, 0, 0, 0);
  await open(page, {
    ...flat,
    expenses: [],
    recurring: [
      {
        id: "rec-rent",
        title: "Rent",
        category: "housing",
        amount_cents: 90000,
        paid_by: "p-alice",
        splits: everyone,
        every: "month",
        start: start.toISOString(),
      },
    ],
  });
  await expect(titles(page)).toHaveText(["Rent", "Rent", "Rent"]);
  await expect(page.getByTestId("expense-item").first()).toContainText("Repeated");

  // Another one, from the form.
  await page.getByRole("button", { name: "Add Expense" }).click();
  const dialog = page.getByRole("dialog", { name: "Add New Expense" });
  await dialog.getByLabel("Description").fill("Internet");
  await dialog.getByLabel("Amount", { exact: true }).fill("30");
  await dialog.getByRole("button", { name: "Repeat" }).click();
  await page.getByRole("menuitemradio", { name: "Every month" }).click();
  await page.keyboard.press("Escape");
  await expect(dialog.getByRole("button", { name: "Repeat: Every month" })).toBeVisible();
  await expect(dialog.getByText("Added again by itself")).toBeVisible();
  await dialog.getByRole("button", { name: "Save Expense" }).click();
  await expect(dialog).not.toBeVisible();
  await expect(titles(page)).toHaveText(["Internet", "Rent", "Rent", "Rent"]);

  await openFromMenu(page, "Repeated expenses");
  const repeated = page.getByRole("dialog", { name: "Repeated expenses" });
  await expect(repeated.getByRole("listitem")).toHaveText([
    /Rent.*€900.*Every month · next on/,
    /Internet.*€30.*Every month · next on/,
  ]);
  await repeated.getByRole("button", { name: "Stop repeating Rent" }).click();
  await page.getByRole("alertdialog").getByRole("button", { name: "Stop" }).click();
  await expect(repeated.getByRole("listitem")).toHaveText([/Internet/]);
  // The ones already added stay.
  await page.keyboard.press("Escape");
  await expect(titles(page)).toHaveText(["Internet", "Rent", "Rent", "Rent"]);
});

test("the activity says who added, edited and deleted what", async ({ page }) => {
  await open(page);
  await page.getByRole("button", { name: "Add Expense" }).click();
  const add = page.getByRole("dialog", { name: "Add New Expense" });
  await add.getByLabel("Description").fill("Taxi");
  await add.getByLabel("Amount", { exact: true }).fill("20");
  await add.getByRole("button", { name: "Save Expense" }).click();
  await expect(add).not.toBeVisible();

  await page.getByRole("button", { name: "Actions for Taxi" }).click();
  await page.getByRole("menuitem", { name: "Edit" }).click();
  const edit = page.getByRole("dialog", { name: "Edit Expense" });
  await edit.getByLabel("Amount", { exact: true }).fill("25");
  await edit.getByRole("button", { name: "Save Changes" }).click();
  await expect(edit).not.toBeVisible();

  await page.getByRole("button", { name: "Actions for Taxi" }).click();
  await page.getByRole("menuitem", { name: "Delete" }).click();
  await expect(titles(page)).toHaveText(["Dinner"]);

  await openFromMenu(page, "Activity");
  const activity = page.getByRole("dialog", { name: "Group activity" });
  await expect(activity.getByRole("listitem")).toHaveText([
    /Alice deleted Taxi/,
    /Alice edited Taxi/,
    /Alice added Taxi.*€20/,
    /Dinner was added.*€60/,
    /Alice and Bob are in the group since it was created/,
  ]);

  // Only who came and went.
  await activity.getByRole("tab", { name: "Members" }).click();
  await expect(activity.getByRole("listitem")).toHaveText([
    /Alice and Bob are in the group since it was created/,
  ]);
});
