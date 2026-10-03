import { expect, type Page, test } from "@playwright/test";
import { installTauriMock } from "./fixtures/tauri-mock";

const day = (n: number) => new Date(Date.UTC(2026, 7, n, 12)).toISOString();
const everyone = ["p-alice", "p-bob", "p-carol"].map((id) => ({ participant_id: id, shares: 1 }));

const expense = (id: string, title: string, cents: number, paidBy: string, date: number) => ({
  id,
  group_id: "group-trip",
  title,
  amount_cents: cents,
  paid_by: paidBy,
  splits: everyone,
  created_at: day(date),
  updated_at: day(date),
  history: [],
  is_reimbursement: false,
});

const trip = {
  id: "group-trip",
  name: "Lisbon Trip",
  currency: "EUR",
  created_at: day(1),
  participants: [
    { id: "p-alice", name: "Alice" },
    { id: "p-bob", name: "Bob" },
    { id: "p-carol", name: "Carol" },
  ],
  expenses: [
    expense("e-cafe", "Café du matin", 1200, "p-alice", 10),
    expense("e-taxi", "Taxi", 4500, "p-bob", 11),
    expense("e-dinner", "Dinner", 9000, "p-alice", 12),
    {
      ...expense("e-museum", "Museum", 3000, "p-carol", 13),
      splits: [{ participant_id: "p-carol", shares: 1 }],
    },
    {
      ...expense("e-payment", "Payment: Bob → Alice", 2000, "p-bob", 14),
      splits: [{ participant_id: "p-alice", shares: 1 }],
      is_reimbursement: true,
    },
  ],
};

test.beforeEach(async ({ page }) => {
  page.on("pageerror", (err) => console.error(">>> BROWSER ERROR:", err));
  await page.addInitScript(installTauriMock);
  await page.addInitScript((d) => Object.assign(window, d), { __SEED_GROUPS__: [trip] });
  await page.goto("/");
  await page.getByRole("button", { name: /Lisbon Trip/ }).click();
});

const titles = (page: Page) => page.getByTestId("expense-item").getByRole("heading");

/** Opens one of the menu's choices ("Sort by", "Show", "Involving") and picks a value. */
async function choose(page: Page, section: string, option: string) {
  await page.getByRole("button", { name: "Filter and sort" }).click();
  await page.getByRole("menuitem", { name: new RegExp(`^${section}`) }).click();
  await page.getByRole("menuitemradio", { name: option, exact: true }).click();
  // The choice closes, and its summary shows what was picked.
  await expect(page.getByRole("menuitem", { name: new RegExp(`^${section}`) })).toContainText(
    option
  );
  await page.keyboard.press("Escape");
}

test("searches by title, payer and amount", async ({ page }) => {
  const search = page.getByRole("searchbox", { name: "Search transactions" });
  await expect(titles(page)).toHaveCount(5);

  // Accents and case don't matter.
  await search.fill("cafe");
  await expect(titles(page)).toHaveText(["Café du matin"]);
  await expect(page.getByText("1 transaction · €12")).toBeVisible();

  await search.fill("carol");
  await expect(titles(page)).toHaveText(["Museum"]);

  await search.fill("45.00");
  await expect(titles(page)).toHaveText(["Taxi"]);

  await search.fill("nothing like this");
  await expect(page.getByText("No transaction matches")).toBeVisible();
  await page.getByRole("button", { name: "Clear filters" }).last().click();
  await expect(titles(page)).toHaveCount(5);
  await expect(search).toHaveValue("");
});

test("filters by kind and by person", async ({ page }) => {
  await choose(page, "Show", "Reimbursements");
  await expect(titles(page)).toHaveText(["Payment: Bob → Alice"]);

  await choose(page, "Show", "Expenses");
  await expect(titles(page)).toHaveCount(4);
  // What the matching expenses add up to.
  await expect(page.getByText("4 transactions")).toBeVisible();

  // Carol paid the museum for herself, and shares the three others; Bob isn't in the museum.
  await choose(page, "Involving", "Bob");
  await expect(titles(page)).toHaveText(["Dinner", "Taxi", "Café du matin"]);

  await page.getByRole("button", { name: "Clear filters" }).click();
  await expect(titles(page)).toHaveCount(5);
});

test("sorts by date, amount and title", async ({ page }) => {
  const newest = ["Payment: Bob → Alice", "Museum", "Dinner", "Taxi", "Café du matin"];
  await expect(titles(page)).toHaveText(newest);

  await choose(page, "Sort by", "Oldest first");
  await expect(titles(page)).toHaveText([...newest].reverse());

  // Sorted another way, the list is no longer cut by day, and each line has its date.
  await choose(page, "Sort by", "Highest amount");
  await expect(titles(page)).toHaveText([
    "Dinner",
    "Taxi",
    "Museum",
    "Payment: Bob → Alice",
    "Café du matin",
  ]);
  await expect(page.getByTestId("expense-item").first()).toContainText("12");

  await choose(page, "Sort by", "Lowest amount");
  await expect(titles(page)).toHaveText([
    "Café du matin",
    "Payment: Bob → Alice",
    "Museum",
    "Taxi",
    "Dinner",
  ]);

  await choose(page, "Sort by", "Title (A to Z)");
  await expect(titles(page)).toHaveText([
    "Café du matin",
    "Dinner",
    "Museum",
    "Payment: Bob → Alice",
    "Taxi",
  ]);
});

test("several people pay one expense", async ({ page }) => {
  await page.getByRole("button", { name: "Add Expense" }).click();
  const dialog = page.getByRole("dialog", { name: "Add New Expense" });
  await dialog.getByLabel("Description").fill("Hotel");
  await dialog.getByLabel("Amount", { exact: true }).fill("300");
  await dialog.getByLabel("Paid by", { exact: true }).click();
  await page.getByRole("option", { name: "Several people…" }).click();

  // Whoever was paying alone starts with the whole amount. From there the amount is what the
  // payers paid between them, and can't be typed.
  const amount = dialog.getByLabel("Amount", { exact: true });
  const paidBy = (name: string) => dialog.getByLabel(`Amount paid by ${name}`);
  await expect(paidBy("Alice")).toHaveValue("300");
  await expect(amount).not.toBeEditable();
  await paidBy("Alice").fill("150");
  await dialog.getByLabel("Bob paid part of it").click();
  await paidBy("Bob").fill("100");
  await expect(amount).toHaveValue("250");
  await dialog.getByLabel("Carol paid part of it").click();

  // Someone ticked needs an amount.
  await dialog.getByRole("button", { name: "Save Expense" }).click();
  await expect(dialog.getByText("Enter what each payer paid, or untick them.")).toBeVisible();
  await paidBy("Carol").fill("50.50");
  await expect(amount).toHaveValue("300.50");
  await paidBy("Carol").fill("50");
  await expect(amount).toHaveValue("300");
  await dialog.getByRole("button", { name: "Save Expense" }).click();
  await expect(dialog).not.toBeVisible();

  const hotel = page.getByTestId("expense-item").filter({ hasText: "Hotel" });
  await expect(hotel).toContainText("Paid by Alice, Bob, and Carol");

  // Each is credited what they paid: Alice paid 12 + 90 + 150, and owes a third of 357.
  await page.getByRole("tab", { name: /Balances/ }).click();
  const alice = page.getByTestId("balance-card").filter({ hasText: "Alice" });
  await expect(alice).toContainText("Paid: €252");

  // Editing shows the payers again, and going back to one person works.
  await page.getByRole("tab", { name: /Expenses/ }).click();
  await page.getByRole("button", { name: "Actions for Hotel" }).click();
  await page.getByRole("menuitem", { name: "Edit" }).click();
  const edit = page.getByRole("dialog", { name: "Edit Expense" });
  await expect(edit.getByLabel("Amount paid by Bob")).toHaveValue("100");
  await edit.getByLabel("Paid by", { exact: true }).click();
  await page.getByRole("option", { name: "Carol" }).click();
  // One payer again: the amount is typed.
  await expect(edit.getByLabel("Amount", { exact: true })).toBeEditable();
  await edit.getByRole("button", { name: "Save Changes" }).click();
  await expect(edit).not.toBeVisible();
  await expect(hotel).toContainText("Paid by Carol");
});

test("exporting on a computer says where the file went", async ({ page }) => {
  await page.addInitScript(() => Object.assign(window, { __NATIVE__: { save: true } }));
  await page.goto("/");
  await page.getByRole("button", { name: "Group options" }).click();
  await page.getByRole("menuitem", { name: "Export as CSV" }).click();

  await expect(page.getByText("Group exported")).toBeVisible();
  await expect(page.getByText("C:\\Users\\alice\\Downloads\\Lisbon Trip.csv")).toBeVisible();
  await expect(page.getByRole("button", { name: "Show the file" })).toBeVisible();
  const saved = await page.evaluate(() => (window as any).__saved);
  expect(saved).toHaveLength(1);
  expect(saved[0].name).toBe("Lisbon Trip.csv");
  expect(saved[0].text).toContain("Café du matin");
});
