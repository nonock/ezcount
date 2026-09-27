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
      ],
      created_at: now,
      updated_at: now,
      history: [],
      is_reimbursement: false,
    },
    {
      id: "exp-payment",
      group_id: "group-trip",
      title: "Payment: Bob → Alice (cash)",
      amount_cents: 2000,
      paid_by: "p-bob",
      splits: [{ participant_id: "p-alice", shares: 1 }],
      created_at: now,
      updated_at: now,
      history: [],
      is_reimbursement: true,
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

test("renaming the group and changing its currency", async ({ page }) => {
  await seed(page, { __SEED_GROUPS__: [tripGroup] });
  await page.goto("/");
  await page.getByRole("button", { name: /Lisbon Trip/ }).click();

  await page.getByRole("button", { name: "Group options" }).click();
  await page.getByRole("menuitem", { name: "Edit group" }).click();
  const dialog = page.getByRole("dialog", { name: "Edit Group" });
  await dialog.getByLabel("Group name").fill("Porto Trip");
  await dialog.getByRole("combobox", { name: "Currency" }).click();
  await page.getByRole("option", { name: /USD/ }).click();
  await dialog.getByRole("button", { name: "Save" }).click();

  await expect(dialog).not.toBeVisible();
  await expect(page.getByRole("heading", { name: "Porto Trip" })).toBeVisible();
  // Amounts keep their value in the new currency.
  await expect(page.getByTestId("expense-item").filter({ hasText: "Dinner" })).toContainText(
    "$90.00"
  );

  await page.getByRole("button", { name: "Back to All Groups" }).click();
  await expect(page.getByRole("button", { name: /Porto Trip/ })).toBeVisible();
});

test("renaming a member renames them everywhere", async ({ page }) => {
  await seed(page, { __SEED_GROUPS__: [tripGroup] });
  await page.goto("/");
  await page.getByRole("button", { name: /Lisbon Trip/ }).click();

  await page.getByRole("button", { name: "Rename Bob" }).click();
  const dialog = page.getByRole("dialog", { name: "Rename Bob" });
  await expect(dialog.getByLabel("Name")).toHaveValue("Bob");
  await dialog.getByLabel("Name").fill("Robert");
  await dialog.getByRole("button", { name: "Rename" }).click();

  await expect(dialog).not.toBeVisible();
  await expect(page.getByRole("button", { name: "Rename Robert" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Rename Bob" })).not.toBeVisible();
  const items = page.getByTestId("expense-item");
  await expect(items.filter({ hasText: "Dinner" })).toContainText("Robert");
  // The payment's title carried the old name.
  await expect(page.getByRole("heading", { name: "Payment: Robert → Alice (cash)" })).toBeVisible();
});

test("Back leaves a group for the list, after closing a dialog", async ({ page }) => {
  await seed(page, { __SEED_GROUPS__: [tripGroup] });
  await page.goto("/");
  const groupList = page.getByRole("heading", { name: "Your Groups" });
  await page.getByRole("button", { name: /Lisbon Trip/ }).click();
  await expect(groupList).not.toBeVisible();

  await page.getByRole("button", { name: "Add Expense", exact: true }).click();
  await expect(page.getByRole("dialog")).toBeVisible();
  await page.goBack();
  await expect(page.getByRole("dialog")).not.toBeVisible();
  await expect(page.getByRole("button", { name: "Back to All Groups" })).toBeVisible();

  await page.goBack();
  await expect(groupList).toBeVisible();

  // The app's own way back doesn't leave an extra step in the history.
  await page.getByRole("button", { name: /Lisbon Trip/ }).click();
  await page.getByRole("button", { name: "Back to All Groups" }).click();
  await expect(groupList).toBeVisible();
  await page.goForward();
  await expect(page.getByRole("button", { name: "Back to All Groups" })).toBeVisible();
});

test("on a phone: the logo and tabs at the bottom, settings in the menu", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await seed(page, { __SEED_GROUPS__: [tripGroup] });
  await page.goto("/");

  const bar = page.getByRole("navigation", { name: "App" });
  const atBottom = async () => {
    const box = await bar.boundingBox();
    return box && Math.round(box.y + box.height);
  };
  expect(await atBottom()).toBe(844);
  await expect(bar.getByRole("button", { name: "New Group" })).toBeVisible();

  await page.getByRole("button", { name: /Lisbon Trip/ }).click();
  expect(await atBottom()).toBe(844);
  await expect(bar.getByRole("tab", { name: "Settle Up" })).toBeVisible();
  // Add Expense floats just above the bar, on the right.
  const add = await page.getByRole("button", { name: "Add Expense", exact: true }).boundingBox();
  const barTop = (await bar.boundingBox())?.y ?? 0;
  expect(add && add.x + add.width).toBeGreaterThan(390 - 32);
  expect(add && barTop - (add.y + add.height)).toBeGreaterThanOrEqual(0);
  expect(add && barTop - (add.y + add.height)).toBeLessThan(32);
  await bar.getByRole("button", { name: "Groups" }).click();
  await expect(page.getByRole("heading", { name: "Your Groups" })).toBeVisible();

  await page.getByRole("button", { name: "Menu" }).click();
  await expect(page.getByRole("menuitemradio", { name: "Dark" })).toBeVisible();
  await expect(page.getByRole("menuitem", { name: "Log out" })).toBeVisible();
});

test("the help on Balances and Settle Up is behind a ?", async ({ page }) => {
  await seed(page, { __SEED_GROUPS__: [tripGroup] });
  await page.goto("/");
  await page.getByRole("button", { name: /Lisbon Trip/ }).click();

  await page.getByRole("tab", { name: "Balances" }).click();
  const help = page.getByText("Positive amounts in");
  await expect(help).not.toBeVisible();
  await page.getByRole("button", { name: "About balances" }).click();
  await expect(help).toBeVisible();
  await page.keyboard.press("Escape");

  await page.getByRole("tab", { name: "Settle Up" }).click();
  await expect(page.getByText("The fewest direct payments")).not.toBeVisible();
  await page.getByRole("button", { name: "About optimal settlement plan" }).click();
  await expect(page.getByText("The fewest direct payments")).toBeVisible();
});
