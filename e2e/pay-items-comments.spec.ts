import { readFileSync } from "node:fs";
import { expect, type Page, test } from "@playwright/test";
import { installTauriMock } from "./fixtures/tauri-mock";

const created = "2026-01-05T12:00:00.000Z";
const everyone = ["p-alice", "p-bob"].map((id) => ({ participant_id: id, shares: 1 }));
const dinner = {
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
};

const flat = {
  id: "group-flat",
  name: "Flat",
  currency: "EUR",
  created_at: created,
  participants: [
    { id: "p-alice", name: "Alice" },
    { id: "p-bob", name: "Bob" },
  ],
  expenses: [dinner],
};

async function seed(page: Page, data: object) {
  page.on("pageerror", (err) => console.error(">>> BROWSER ERROR:", err));
  await page.addInitScript(installTauriMock);
  await page.addInitScript((d) => Object.assign(window, d), data);
  await page.goto("/");
}

async function open(page: Page, group: object = flat) {
  await seed(page, { __SEED_GROUPS__: [group] });
  await page.getByRole("button", { name: /Flat/ }).click();
}

test("an IBAN in the profile lets the others pay by scanning a code", async ({ page }) => {
  await open(page);
  await page.getByRole("button", { name: "Menu" }).click();
  await page.getByRole("menuitem", { name: /Account/ }).click();
  const account = page.getByRole("dialog");
  await account.getByLabel("IBAN").fill("FR76 3000 6000 0112 3456 7890 188");
  await account.getByRole("button", { name: "Save profile" }).click();
  await expect(account.getByText("This IBAN is not valid.")).toBeVisible();
  await account.getByLabel("IBAN").fill("fr76 3000 6000 0112 3456 7890 189");
  await account.getByRole("button", { name: "Save profile" }).click();
  await expect(page.getByText("Profile saved")).toBeVisible();
  await page.keyboard.press("Escape");

  // Bob owes Alice, who now has an account to be paid on.
  await page.getByRole("tab", { name: /Settle/ }).click();
  await page.getByRole("button", { name: "Pay Alice by bank transfer" }).click();
  const pay = page.getByRole("dialog");
  await expect(pay.getByRole("heading", { name: "Pay Alice" })).toBeVisible();
  await expect(pay.getByTestId("transfer-iban")).toHaveText("FR76 3000 6000 0112 3456 7890 189");
  await expect(pay.getByRole("img", { name: "QR code of a bank transfer to Alice" })).toBeVisible();
  await expect(pay).toContainText("€30");

  // Once paid, it is recorded as usual.
  await pay.getByRole("button", { name: "Mark as Paid" }).click();
  const record = page.getByRole("dialog");
  await expect(record.getByRole("heading", { name: "Record Reimbursement" })).toBeVisible();
  await expect(record.getByLabel("Amount", { exact: true })).toHaveValue(/^30(.00)?$/);
});

test("a group in another currency has no code, only the IBAN", async ({ page }) => {
  const alice = { id: "p-alice", name: "Alice", iban: "DE89370400440532013000" };
  await open(page, { ...flat, currency: "USD", participants: [alice, flat.participants[1]] });
  await page.getByRole("tab", { name: /Settle/ }).click();
  await page.getByRole("button", { name: "Pay Alice by bank transfer" }).click();
  const pay = page.getByRole("dialog");
  await expect(pay.getByTestId("transfer-iban")).toHaveText("DE89 3704 0044 0532 0130 00");
  await expect(pay.getByRole("img")).toHaveCount(0);
});

test("the group list says what the user owes and is owed overall", async ({ page }) => {
  const trip = {
    ...flat,
    id: "group-trip",
    name: "Trip",
    expenses: [
      { ...dinner, id: "e-taxi", group_id: "group-trip", paid_by: "p-bob", amount_cents: 2000 },
    ],
  };
  await seed(page, { __SEED_GROUPS__: [flat, trip] });
  const overall = page.getByRole("region", { name: "Your balance over all your groups" });
  await expect(overall).toHaveText(/You are owed\s*You owe\s*€30\s*€10/);
  const nets = page.getByTestId("group-net");
  await expect(nets.nth(0)).toHaveText(/You get back\s*€30/);
  await expect(nets.nth(1)).toHaveText(/You owe\s*€10/);
});

test("an expense entered item by item gives each person what they took", async ({ page }) => {
  await open(page, { ...flat, expenses: [] });
  await page.getByRole("button", { name: "Add Expense" }).click();
  const dialog = page.getByRole("dialog");
  await dialog.getByLabel("Description").fill("Groceries");
  await dialog.getByLabel("Amount", { exact: true }).fill("12");
  await dialog.getByRole("button", { name: "Split by item…" }).click();

  // The first line starts as the whole expense; the wine was Bob's alone.
  await expect(dialog.getByLabel("Amount of item 1")).toHaveValue(/^12(.00)?$/);
  await dialog.getByLabel("Name of item 1").fill("Wine");
  const forWine = dialog.getByRole("group", { name: "Who item 1 is for" });
  await forWine.getByRole("button", { name: "Alice" }).click();
  await expect(forWine.getByRole("button", { name: "Alice" })).toHaveAttribute(
    "aria-pressed",
    "false"
  );
  await dialog.getByRole("button", { name: "Add an item" }).click();
  await dialog.getByLabel("Name of item 2").fill("Bread");
  await dialog.getByRole("button", { name: "Save Expense" }).click();
  await expect(dialog.getByText("Enter an amount for each item, or remove it.")).toBeVisible();
  await dialog.getByLabel("Amount of item 2").fill("3.01");

  // The amount is the lines' total, and the odd cent goes to the first on the line.
  await expect(dialog.getByLabel("Amount", { exact: true })).toHaveValue("15.01");
  const owed = dialog.getByRole("list", { name: "What each person owes" });
  await expect(owed).toContainText("Bob €13.50");
  await expect(owed).toContainText("Alice €1.51");
  await dialog.getByRole("button", { name: "Save Expense" }).click();
  await expect(dialog).not.toBeVisible();

  const item = page.getByTestId("expense-item");
  await expect(item).toContainText("Bob €13.50");
  await expect(item).toContainText("Alice €1.51");

  // Editing it shows the lines again.
  await item.getByRole("button", { name: "Actions for Groceries" }).click();
  await page.getByRole("menuitem", { name: "Edit" }).click();
  await expect(dialog.getByLabel("Name of item 2")).toHaveValue("Bread");
  await dialog.getByRole("button", { name: "Remove item 2" }).click();
  await expect(dialog.getByLabel("Amount", { exact: true })).toHaveValue(/^12(.00)?$/);
  await dialog.getByRole("button", { name: "Save Changes" }).click();
  await expect(item).toContainText("Bob €12");
});

test("members comment an expense, which the activity lists", async ({ page }) => {
  await open(page);
  const item = page.getByTestId("expense-item");
  await item.getByRole("button", { name: "Actions for Dinner" }).click();
  await page.getByRole("menuitem", { name: "Comments" }).click();
  const dialog = page.getByRole("dialog");
  await expect(dialog.getByText("No comment yet.")).toBeVisible();
  await dialog.getByLabel("Your comment").fill("With the tip");
  await dialog.getByRole("button", { name: "Send" }).click();
  const comments = dialog.getByRole("list", { name: "Comments" });
  await expect(comments).toContainText("Alice");
  await expect(comments).toContainText("With the tip");
  await expect(dialog.getByLabel("Your comment")).toHaveValue("");
  await page.keyboard.press("Escape");

  await page.getByRole("button", { name: "Activity and trash" }).click();
  await page.getByRole("menuitem", { name: "Activity" }).click();
  const activity = page.getByRole("list", { name: "Group activity" });
  await expect(activity.getByRole("listitem").first()).toContainText("Alice commented on Dinner");
  await expect(activity.getByRole("listitem").first()).toContainText("With the tip");
  await page.keyboard.press("Escape");

  // The count on the expense leads back to them; one's own can be removed.
  await item.getByRole("button", { name: "1 comment on Dinner" }).click();
  await dialog.getByRole("button", { name: "Delete this comment" }).click();
  await expect(dialog.getByText("No comment yet.")).toBeVisible();
});

test("the trash lets go of what was deleted over 30 days ago", async ({ page }) => {
  const days = (n: number) => new Date(Date.now() - n * 24 * 3600 * 1000).toISOString();
  const deleted = (title: string, at: string) => ({
    expense: { ...dinner, id: `e-${title}`, title },
    deleted_at: at,
    deleted_by: "p-bob",
  });
  await open(page, {
    ...flat,
    trash: [deleted("Taxi", days(2)), deleted("Museum", days(31))],
  });
  await page.getByRole("button", { name: "Activity and trash" }).click();
  await page.getByRole("menuitem", { name: /Trash/ }).click();
  const trash = page.getByRole("list", { name: "Trash" });
  await expect(trash.getByRole("listitem")).toHaveCount(1);
  await expect(trash).toContainText("Taxi");
  await expect(page.getByRole("dialog")).toContainText("for 30 days");
});

test("exporting a group's summary as a PDF file", async ({ page }) => {
  await open(page);
  await page.getByRole("button", { name: "Group options" }).click();
  const download = page.waitForEvent("download");
  await page.getByRole("menuitem", { name: "Export as PDF" }).click();
  const file = await download;
  expect(file.suggestedFilename()).toBe("Flat.pdf");
  const pdf = readFileSync(await file.path());
  expect(pdf.subarray(0, 5).toString()).toBe("%PDF-");
  expect(pdf.length).toBeGreaterThan(2000);
  await expect(page.getByText("Group exported")).toBeVisible();
});

test("on a computer the PDF goes to the Downloads folder", async ({ page }) => {
  await seed(page, { __SEED_GROUPS__: [flat], __NATIVE__: { save: true } });
  await page.getByRole("button", { name: /Flat/ }).click();
  await page.getByRole("button", { name: "Group options" }).click();
  await page.getByRole("menuitem", { name: "Export as PDF" }).click();
  await expect(page.getByText("Flat.pdf")).toBeVisible();
  const saved = await page.evaluate(() => (window as any).__saved[0]);
  expect(saved.name).toBe("Flat.pdf");
  // "%PDF"
  expect(saved.bytes.slice(0, 4)).toEqual([37, 80, 68, 70]);
});

test("an idea is sent from the menu, without another account", async ({ page }) => {
  await seed(page, { __SEED_GROUPS__: [flat] });
  await page.getByRole("button", { name: "Menu" }).click();
  await page.getByRole("menuitem", { name: "Suggest a feature" }).click();
  const dialog = page.getByRole("dialog");
  await expect(dialog.getByRole("button", { name: "Send" })).toBeDisabled();
  await dialog.getByLabel("Your message").fill("Budgets per category, please");
  await dialog.getByLabel("E-mail (optional)").fill("alice@example.com");
  await dialog.getByRole("button", { name: "Send" }).click();
  await expect(page.getByText("Thank you, your message was sent")).toBeVisible();
  await expect(dialog).not.toBeVisible();
  const sent = await page.evaluate(() => (window as any).__feedback[0]);
  expect(sent.message).toBe("Budgets per category, please");
  expect(sent.contact).toBe("alice@example.com");
  expect(sent.app).toContain("ezcount 0.0.0-test");
});

test("a server from before the form says it can't take the message", async ({ page }) => {
  await seed(page, { __SEED_GROUPS__: [flat], __OLD_RELAY__: true });
  await page.getByRole("button", { name: "Menu" }).click();
  await page.getByRole("menuitem", { name: "Suggest a feature" }).click();
  const dialog = page.getByRole("dialog");
  await dialog.getByLabel("Your message").fill("Hello");
  await dialog.getByRole("button", { name: "Send" }).click();
  await expect(dialog.getByText("This sync server doesn't take messages yet")).toBeVisible();
});
