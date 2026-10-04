import { expect, type Page, test } from "@playwright/test";
import { installTauriMock } from "./fixtures/tauri-mock";

const day = (month: number, n: number) => new Date(Date.UTC(2026, month - 1, n, 12)).toISOString();
const everyone = ["p-alice", "p-bob"].map((id) => ({ participant_id: id, shares: 1 }));

const expense = (
  id: string,
  title: string,
  cents: number,
  paidBy: string,
  date: string,
  category?: string
) => ({
  id,
  group_id: "group-trip",
  title,
  category,
  amount_cents: cents,
  paid_by: paidBy,
  splits: everyone,
  created_at: date,
  updated_at: date,
  history: [],
  is_reimbursement: false,
});

const trip = {
  id: "group-trip",
  name: "Lisbon Trip",
  currency: "EUR",
  created_at: day(7, 1),
  participants: [
    { id: "p-alice", name: "Alice" },
    { id: "p-bob", name: "Bob" },
  ],
  expenses: [
    expense("e-dinner", "Dinner", 6000, "p-alice", day(7, 10), "food"),
    expense("e-lunch", "Lunch", 2000, "p-bob", day(8, 2), "food"),
    expense("e-taxi", "Taxi", 1500, "p-bob", day(8, 3), "transport"),
    expense("e-misc", "Misc", 500, "p-alice", day(8, 4)),
    {
      ...expense("e-payment", "Payment: Bob → Alice", 1000, "p-bob", day(8, 5)),
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

test("an expense is filed under a category, which can change", async ({ page }) => {
  await page.getByRole("button", { name: "Add Expense" }).click();
  const dialog = page.getByRole("dialog", { name: "Add New Expense" });
  await dialog.getByLabel("Description").fill("Museum");
  await dialog.getByLabel("Amount", { exact: true }).fill("30");
  await dialog.getByLabel("Category").click();
  await page.getByRole("option", { name: "Leisure" }).click();
  await dialog.getByRole("button", { name: "Save Expense" }).click();
  await expect(dialog).not.toBeVisible();

  // The list shows the category in place of the title's initial.
  const museum = page.getByTestId("expense-item").filter({ hasText: "Museum" });
  await expect(museum.getByText("Leisure")).toBeAttached();

  // Editing shows it, and it can be taken off.
  await page.getByRole("button", { name: "Actions for Museum" }).click();
  await page.getByRole("menuitem", { name: "Edit" }).click();
  const edit = page.getByRole("dialog", { name: "Edit Expense" });
  await expect(edit.getByLabel("Category")).toContainText("Leisure");
  await edit.getByLabel("Category").click();
  await page.getByRole("option", { name: "No category" }).click();
  await edit.getByRole("button", { name: "Save Changes" }).click();
  await expect(edit).not.toBeVisible();
  await expect(museum.getByText("Leisure")).toHaveCount(0);
});

test("the list is filtered by category, and the search knows categories", async ({ page }) => {
  await page.getByRole("button", { name: "Filter and sort" }).click();
  await page.getByRole("menuitem", { name: /^Category/ }).click();
  // Only the categories the group uses are offered.
  await expect(page.getByRole("menuitemradio")).toHaveText([
    "All categories",
    "Restaurants",
    "Transport",
    "No category",
  ]);
  await page.getByRole("menuitemradio", { name: "Restaurants" }).click();
  await page.keyboard.press("Escape");
  await expect(titles(page)).toHaveText(["Lunch", "Dinner"]);

  await page.getByRole("button", { name: "Clear filters" }).click();
  await page.getByRole("searchbox", { name: "Search transactions" }).fill("transport");
  await expect(titles(page)).toHaveText(["Taxi"]);
});

test("statistics count the spending by category, person and month", async ({ page }) => {
  await page.getByRole("tab", { name: /Stats/ }).click();

  // 100.00 spent in four expenses; the payment isn't one.
  const panel = page.getByRole("tabpanel");
  await expect(panel).toContainText("Total spent €100");
  await expect(panel).toContainText("Expenses 4");
  await expect(panel).toContainText("Average expense €25");

  const categories = page.getByTestId("stats-categories").getByRole("listitem");
  await expect(categories).toHaveCount(3);
  await expect(categories.nth(0)).toContainText("Restaurants");
  await expect(categories.nth(0)).toContainText("2 expenses");
  await expect(categories.nth(0)).toContainText("€80");
  await expect(categories.nth(0)).toContainText("80%");
  await expect(categories.nth(1)).toContainText("Transport");
  await expect(categories.nth(1)).toContainText("15%");
  await expect(categories.nth(2)).toContainText("No category");
  await expect(categories.nth(2)).toContainText("5%");

  // Everything is shared equally: half each, whatever each paid.
  const people = page.getByTestId("stats-people").getByRole("listitem");
  await expect(people.filter({ hasText: "Alice" })).toContainText("€50");
  await expect(people.filter({ hasText: "Alice" })).toContainText("Paid €65");
  await expect(people.filter({ hasText: "Bob" })).toContainText("50%");
  await expect(people.filter({ hasText: "Bob" })).toContainText("Paid €35");

  const months = page.getByTestId("stats-months").getByRole("listitem");
  await expect(months).toHaveCount(2);
  await expect(months.nth(0)).toContainText("August 2026");
  await expect(months.nth(0)).toContainText("€40");
  await expect(months.nth(1)).toContainText("July 2026");
  await expect(months.nth(1)).toContainText("€60");
});

test("the activity says who was added and removed, and by whom", async ({ page }) => {
  await page.getByRole("button", { name: "Add Member" }).click();
  const add = page.getByRole("dialog", { name: "Add Group Member" });
  await add.getByLabel("Name").fill("Carol");
  await add.getByRole("button", { name: "Add to Group" }).click();
  await expect(add).not.toBeVisible();

  await page.getByRole("button", { name: "Rename or remove Carol" }).click();
  await page.getByRole("button", { name: "Remove from group" }).click();
  await page.getByRole("alertdialog").getByRole("button", { name: "Remove" }).click();

  await page.getByRole("button", { name: "Activity and trash" }).click();
  await page.getByRole("menuitem", { name: "Activity" }).click();
  const history = page.getByRole("dialog", { name: "Group activity" });
  await history.getByRole("tab", { name: "Members" }).click();
  // The latest first, then the members the group started with.
  await expect(history.getByRole("listitem")).toHaveText([
    /Alice removed Carol/,
    /Alice added Carol/,
    /Alice and Bob are in the group since it was created/,
  ]);
});
