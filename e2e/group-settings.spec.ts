import { readFileSync } from "node:fs";
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
  await dialog.getByLabel("Currency", { exact: true }).click();
  await page.getByRole("option", { name: /USD/ }).click();
  await dialog.getByRole("button", { name: "Save" }).click();

  await expect(dialog).not.toBeVisible();
  await expect(page.getByRole("heading", { name: "Porto Trip" })).toBeVisible();
  // Amounts keep their value in the new currency.
  await expect(page.getByTestId("expense-item").filter({ hasText: "Dinner" })).toContainText("$90");

  await page.getByRole("button", { name: "Back to All Groups" }).click();
  await expect(page.getByRole("button", { name: /Porto Trip/ })).toBeVisible();
});

test("renaming a member renames them everywhere", async ({ page }) => {
  await seed(page, { __SEED_GROUPS__: [tripGroup] });
  await page.goto("/");
  await page.getByRole("button", { name: /Lisbon Trip/ }).click();

  await page.getByRole("button", { name: "Rename or remove Bob" }).click();
  const dialog = page.getByRole("dialog", { name: "Rename Bob" });
  await expect(dialog.getByLabel("Name")).toHaveValue("Bob");
  await dialog.getByLabel("Name").fill("Robert");
  await dialog.getByRole("button", { name: "Rename" }).click();

  await expect(dialog).not.toBeVisible();
  await expect(page.getByRole("button", { name: "Rename or remove Robert" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Rename or remove Bob" })).not.toBeVisible();
  // Shared by everyone, the dinner no longer lists who shares it.
  await expect(page.getByTestId("expense-item").filter({ hasText: "Dinner" })).toContainText(
    "for everyone"
  );
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
  await expect(page.getByRole("menuitem", { name: "Connect a device" })).toBeVisible();
  await expect(page.getByRole("menuitem", { name: /Account/ })).toBeVisible();
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

test("exporting a group as a CSV file", async ({ page }) => {
  await seed(page, { __SEED_GROUPS__: [tripGroup] });
  await page.goto("/");
  await page.getByRole("button", { name: /Lisbon Trip/ }).click();

  await page.getByRole("button", { name: "Group options" }).click();
  const download = page.waitForEvent("download");
  await page.getByRole("menuitem", { name: "Export as CSV" }).click();
  const file = await download;
  expect(file.suggestedFilename()).toBe("Lisbon Trip.csv");
  const lines = readFileSync(await file.path(), "utf8").split("\n");
  // After the byte order mark Excel wants.
  expect(lines[0]).toBe(
    "\ufeffDate,Title,Amount,Currency,Paid by,Type,Original amount,Original currency,Exchange rate,Split,Alice,Bob"
  );
  expect(lines[1]).toBe(`${now},Dinner,90.00,EUR,Alice,expense,,,,1 1,45.00,45.00`);
  expect(lines[2]).toBe(`${now},Payment: Bob → Alice (cash),20.00,EUR,Bob,payment,,,,,20.00,`);
});

test("importing a group from a CSV file", async ({ page }) => {
  await seed(page, { __SEED_GROUPS__: [tripGroup] });
  await page.goto("/");

  const csv = [
    "Date,Title,Amount,Currency,Paid by,Type,Original amount,Original currency,Exchange rate,Split,Ann,Ben",
    "2026-01-02T10:00:00Z,Gift,30.00,USD,Ann,expense,,,,,10.00,20.00",
    "2026-01-03T10:00:00Z,Payment: Ben → Ann,5.00,USD,Ben,payment,,,,,5.00,",
  ].join("\n");
  await expect(page.getByRole("button", { name: "Import CSV" })).toBeVisible();
  await page.getByLabel("CSV file to import").setInputFiles({
    name: "Christmas gifts.csv",
    mimeType: "text/csv",
    buffer: Buffer.from(csv),
  });

  // The file doesn't say who the user is.
  const who = page.getByRole("dialog", { name: 'Who are you in "Christmas gifts"?' });
  await who.getByRole("button", { name: "Ben", exact: true }).click();
  await expect(who).not.toBeVisible();
  await expect(page.getByRole("heading", { name: "Christmas gifts" })).toBeVisible();
  await expect(page.getByTestId("expense-item").filter({ hasText: "Gift" })).toContainText("$30");
  await expect(page.getByTestId("expense-item")).toHaveCount(2);

  // A file that isn't one says so, and nothing is created.
  await page.getByRole("button", { name: "Back to All Groups" }).click();
  await page.getByLabel("CSV file to import").setInputFiles({
    name: "notes.csv",
    mimeType: "text/csv",
    buffer: Buffer.from("Who,What\nAnn,Gift"),
  });
  await expect(page.getByText("Could not import the file")).toBeVisible();
  await expect(page.getByText("This is not an ezcount CSV file")).toBeVisible();
});

test("an expense in another currency, with a set amount for someone", async ({ page }) => {
  await seed(page, {
    __SEED_GROUPS__: [tripGroup],
    __RATES__: { "USD/EUR": "0.9234", "GBP/EUR": "1.15" },
  });
  await page.goto("/");
  await page.getByRole("button", { name: /Lisbon Trip/ }).click();

  await page.getByRole("button", { name: "Add Expense", exact: true }).click();
  const dialog = page.getByRole("dialog", { name: "Add New Expense" });
  await dialog.getByLabel("Description").fill("Taxi");
  await dialog.getByLabel("Amount", { exact: true }).fill("50");
  await dialog.getByLabel("Currency", { exact: true }).click();
  await page.getByRole("option", { name: "USD", exact: true }).click();
  // The relay's rate is suggested; one typed in stays, another currency gets its own.
  await expect(dialog.getByLabel("Exchange rate")).toHaveValue("0.9234");
  await expect(dialog.getByText("Suggested rate")).toBeVisible();
  await dialog.getByLabel("Currency", { exact: true }).click();
  await page.getByRole("option", { name: "GBP", exact: true }).click();
  await expect(dialog.getByLabel("Exchange rate")).toHaveValue("1.15");
  // No suggestion for this one: the field waits for the user.
  await dialog.getByLabel("Currency", { exact: true }).click();
  await page.getByRole("option", { name: "CHF", exact: true }).click();
  await expect(dialog.getByLabel("Exchange rate")).toHaveValue("");
  await dialog.getByLabel("Currency", { exact: true }).click();
  await page.getByRole("option", { name: "USD", exact: true }).click();
  await expect(dialog.getByLabel("Exchange rate")).toHaveValue("0.9234");
  await expect(dialog.getByText("Counts as €46.17 in the group.")).toBeVisible();

  // Bob owes 20 dollars of it; Alice, on parts, the rest.
  await dialog.getByRole("button", { name: "Set an amount for Bob" }).click();
  await dialog.getByLabel("Amount for Bob").fill("70");
  await expect(dialog.getByText("The amounts are $20 more than the expense.")).toBeVisible();
  await dialog.getByLabel("Amount for Bob").fill("20");
  await expect(dialog.getByText("$30", { exact: true })).toBeVisible();
  await dialog.getByRole("button", { name: "Save Expense" }).click();
  await expect(dialog).not.toBeVisible();

  const taxi = page.getByTestId("expense-item").filter({ hasText: "Taxi" });
  await expect(taxi).toContainText("€46.17");
  await expect(taxi).toContainText("$50");
  await expect(taxi.getByLabel("Split between")).toContainText("Bob $20");

  // 20 of the 50 dollars is 18.47 of the 46.17 euros, on top of the 25.00 Bob owed.
  await page.getByRole("tab", { name: "Balances" }).click();
  await expect(page.getByText("-€43.47")).toBeVisible();

  // Editing shows it as it was typed.
  await page.getByRole("tab", { name: "Expenses" }).click();
  await taxi.getByRole("button", { name: /Actions for/ }).click();
  await page.getByRole("menuitem", { name: "Edit" }).click();
  const editing = page.getByRole("dialog", { name: "Edit Expense" });
  await expect(editing.getByLabel("Amount", { exact: true })).toHaveValue("50");
  await expect(editing.getByLabel("Exchange rate")).toHaveValue("0.9234");
  // A saved rate is the expense's own: no longer a suggestion.
  await expect(editing.getByText("Suggested rate")).not.toBeVisible();
  await expect(editing.getByLabel("Amount for Bob")).toHaveValue("20");
});
