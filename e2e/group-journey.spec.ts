import { expect, type Page, test } from "@playwright/test";
import { installTauriMock } from "./fixtures/tauri-mock";

async function chooseOption(page: Page, trigger: string, option: string) {
  await page.locator(trigger).click();
  await page.getByRole("option", { name: option, exact: true }).click();
}

/** Says who the user is in the open group, from the group's menu. */
async function chooseIdentity(page: Page, name: string) {
  await page.getByRole("button", { name: "Group options" }).click();
  await page.getByRole("menuitem", { name: "Change who you are" }).click();
  await page.getByRole("dialog").getByRole("button", { name, exact: true }).click();
  await expect(page.getByText("Your balance", { exact: true })).toBeVisible();
}

/** Picks a day in the expense date picker, moving back a month if it isn't shown. */
async function pickExpenseDate(page: Page, date: Date) {
  await page.locator("#input-expense-date").click();
  // The calendar tags each day with its ISO date.
  const pad = (n: number) => String(n).padStart(2, "0");
  const key = `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
  const day = page.locator(`[data-value="${key}"]`);
  if ((await day.count()) === 0) {
    await page.getByRole("button", { name: "Previous" }).click();
  }
  await day.first().click();
}

test.beforeEach(async ({ page }) => {
  page.on("pageerror", (err) => console.error(">>> BROWSER ERROR:", err));
  // Inject in-memory Tauri IPC mock before any app script loads
  await page.addInitScript(installTauriMock);
});

test.describe("Group Lifecycle & Selection (Regression Test)", () => {
  test("creates a new group, navigates to dashboard, and re-opens group on card click", async ({
    page,
  }) => {
    await page.goto("/");

    // Verify initial empty state
    await expect(page.getByText("No groups yet")).toBeVisible();

    // Open create group modal
    await page.getByRole("button", { name: "Create Group" }).click();
    await expect(page.getByRole("heading", { name: "Create New Group" })).toBeVisible();

    // Fill form
    await page.locator("#input-group-name").fill("Rome Holiday");
    await chooseOption(page, "#select-group-currency", "EUR (€) — Euro");

    // The first person is the user, suggested from the account.
    await expect(page.getByLabel("Your name")).toHaveValue("alice");
    await page.getByLabel("Your name").fill("Alice");
    const participantInputs = page.locator("input[placeholder^='Participant']");
    await participantInputs.nth(0).fill("Bob");
    await participantInputs.nth(1).fill("Charlie");

    // Submit form
    await page.getByRole("button", { name: "Create Group", exact: true }).click();

    // Should immediately navigate into the newly created group workspace
    await expect(page.getByRole("heading", { name: "Rome Holiday" })).toBeVisible();
    await expect(page.getByText("Alice").first()).toBeVisible();
    await expect(page.getByText("Bob").first()).toBeVisible();
    await expect(page.getByText("Charlie").first()).toBeVisible();
    // Reimburse button should be hidden when there are 0 expenses / 0 balance
    await expect(page.getByRole("button", { name: "Reimburse" })).not.toBeVisible();

    // Navigate back to the dashboard
    await page.getByRole("button", { name: "Back to All Groups" }).click();

    // Verify we are back on the dashboard with the group card visible
    await expect(page.getByRole("heading", { name: "Your Groups" })).toBeVisible();
    const groupCard = page.getByRole("button", { name: /Rome Holiday/ });
    await expect(groupCard).toBeVisible();

    // Click the group card (THIS WAS THE PREVIOUS BUG: clicking the card did nothing!)
    await groupCard.click();

    // Verify workspace successfully opens with full group details
    await expect(page.getByRole("heading", { name: "Rome Holiday" })).toBeVisible();
    await expect(page.getByText("Alice").first()).toBeVisible();
    await expect(page.getByText("Bob").first()).toBeVisible();
    await expect(page.getByText("Charlie").first()).toBeVisible();
    await expect(page.getByText("No expenses recorded yet")).toBeVisible();
    await expect(page.getByRole("button", { name: "Reimburse" })).not.toBeVisible();
  });
});

test.describe("Expense & Settlement Lifecycle", () => {
  test("adds expense with custom splits, edits it, checks edit history, and verifies settlements", async ({
    page,
  }) => {
    await page.goto("/");

    // Create a group
    await page.getByRole("button", { name: "Create Group" }).click();
    await page.locator("#input-group-name").fill("Ski Trip 2026");
    await chooseOption(page, "#select-group-currency", "EUR (€) — Euro");

    await page.getByLabel("Your name").fill("Alice");
    await page.locator("input[placeholder^='Participant']").first().fill("Bob");
    // Remove 3rd participant
    await page
      .getByRole("button", { name: /Remove participant/ })
      .last()
      .click();

    await page.getByRole("button", { name: "Create Group", exact: true }).click();
    await expect(page.getByRole("heading", { name: "Ski Trip 2026" })).toBeVisible();
    // 0 expenses -> Reimburse button hidden
    await expect(page.getByRole("button", { name: "Reimburse" })).not.toBeVisible();

    // Add first expense
    await page.getByRole("button", { name: "Add Expense", exact: true }).click();
    await expect(page.getByRole("heading", { name: "Add New Expense" })).toBeVisible();

    await page.locator("#input-expense-title").fill("Chalet Rental");
    await page.locator("#input-expense-amount").fill("200.00");
    // Default payer is Alice
    await page.getByRole("button", { name: "Save Expense" }).click();

    // After expense is recorded with non-zero debt -> Reimburse button becomes visible
    await expect(page.getByRole("button", { name: "Reimburse" })).toBeVisible();

    // Verify expense appears in list under Today group
    await expect(page.getByText("Today")).toBeVisible();
    const expenseCard = page.locator("[data-testid='expense-item']").first();
    await expect(expenseCard).toBeVisible();
    await expect(expenseCard.getByRole("heading", { name: "Chalet Rental" })).toBeVisible();
    await expect(expenseCard.getByText("€200.00")).toBeVisible();
    await expect(expenseCard.getByText("Paid by Alice", { exact: false })).toBeVisible();

    // The creator is Alice, so the summary is hers.
    await expect(page.getByText("Your balance", { exact: true })).toBeVisible();
    await expect(page.getByText("Paid by you")).toBeVisible();
    await expect(page.getByText("€200.00").first()).toBeVisible();
    await expect(page.getByText("+€100.00")).toBeVisible();

    // Saying you're Bob instead updates the summary
    await chooseIdentity(page, "Bob");
    await expect(page.getByText("-€100.00")).toBeVisible();
    await chooseIdentity(page, "Alice");

    // Add a second expense with a past date (yesterday)
    const yesterday = new Date();
    yesterday.setDate(yesterday.getDate() - 1);

    await page.getByRole("button", { name: "Add Expense", exact: true }).click();
    await page.locator("#input-expense-title").fill("Fondue Yesterday");
    await page.locator("#input-expense-amount").fill("50.00");
    await pickExpenseDate(page, yesterday);
    await expect(page.locator("#input-expense-date")).toContainText(
      yesterday.toLocaleDateString("en-US", { dateStyle: "medium" })
    );
    await page.getByRole("button", { name: "Save Expense" }).click();

    // Verify both "Today" and "Yesterday" headers are visible
    await expect(page.getByText("Today", { exact: true })).toBeVisible();
    await expect(page.getByText("Yesterday", { exact: true })).toBeVisible();
    await expect(page.getByRole("heading", { name: "Fondue Yesterday" })).toBeVisible();

    // Edit the first expense
    await expenseCard.getByRole("button", { name: /Actions for/ }).click();
    await page.getByRole("menuitem", { name: "Edit" }).click();
    await expect(page.getByRole("heading", { name: "Edit Expense" })).toBeVisible();

    await page.locator("#input-expense-title").fill("Chalet Rental & Firewood");
    await page.locator("#input-expense-amount").fill("250.00");
    await page.getByRole("button", { name: "Save Changes" }).click();

    // Verify updated details
    await expect(
      expenseCard.getByRole("heading", { name: "Chalet Rental & Firewood" })
    ).toBeVisible();
    await expect(expenseCard.getByText("€250.00")).toBeVisible();

    // Verify "Edited (1)" badge is visible and clickable
    const editedBadge = expenseCard.getByRole("button", { name: /Edited/i });
    await expect(editedBadge).toBeVisible();
    await editedBadge.click();

    // Verify history modal
    await expect(page.getByRole("heading", { name: "Expense Revision History" })).toBeVisible();
    const historyModal = page.getByRole("dialog");
    await expect(historyModal.getByText("Chalet Rental", { exact: true })).toBeVisible();
    await expect(historyModal.getByText("€200.00")).toBeVisible();

    // Close history modal
    await page.getByRole("button", { name: "Close" }).click();

    // Check Balances tab (Alice paid 250 + 50 = 300, total = 300, share = 150 each -> Bob owes 150)
    await page.getByRole("tab", { name: "Balances" }).click();
    await expect(page.getByText("Gets back")).toBeVisible();
    await expect(page.getByText("Owes", { exact: true })).toBeVisible();
    await expect(page.getByText("-€150.00")).toBeVisible();

    // Check Settle Up tab
    await page.getByRole("tab", { name: "Settle Up" }).click();
    const settleCard = page.locator("li").filter({ hasText: "Bob pays Alice" });
    await expect(settleCard).toBeVisible();
    await expect(settleCard.getByText("€150.00")).toBeVisible();
    await expect(settleCard.getByRole("button", { name: "Mark as Paid" })).toBeVisible();
  });

  test("batches transaction history in chunks of 10 with progressive loading", async ({ page }) => {
    const now = new Date();
    const mockExpenses = Array.from({ length: 15 }, (_, i) => {
      const d = new Date(now.getTime() - i * 3600 * 1000);
      return {
        id: `exp-${i + 1}`,
        group_id: "batch-group-1",
        title: `Batch Item #${i + 1}`,
        amount_cents: 1000 + i * 100,
        paid_by: "p-1",
        splits: [
          { participant_id: "p-1", shares: 1 },
          { participant_id: "p-2", shares: 1 },
        ],
        created_at: d.toISOString(),
        updated_at: d.toISOString(),
        history: [],
        is_reimbursement: false,
      };
    });

    const seedGroups = [
      {
        id: "batch-group-1",
        name: "Large Expense Group",
        currency: "EUR",
        participants: [
          { id: "p-1", name: "Alice" },
          { id: "p-2", name: "Bob" },
        ],
        expenses: mockExpenses,
        created_at: now.toISOString(),
      },
    ];

    await page.addInitScript((seed) => {
      (window as any).__SEED_GROUPS__ = seed;
    }, seedGroups);

    await page.goto("/");

    // Click on the seeded group card
    await page.getByRole("heading", { name: "Large Expense Group" }).click();
    await expect(page.getByRole("heading", { name: "Large Expense Group" })).toBeVisible();

    // Verify initial batch of 10 items
    const expenseCards = page.locator("[data-testid='expense-item']");
    await expect(expenseCards).toHaveCount(10);
    await expect(page.getByText("Showing 10 of 15 transactions")).toBeVisible();

    // Load more button should be present
    const loadMoreBtn = page.getByRole("button", { name: /Load 10 more transactions/i });
    await expect(loadMoreBtn).toBeVisible();

    // Click to load next batch
    await loadMoreBtn.click();

    // Should now show all 15 items
    await expect(expenseCards).toHaveCount(15);
    await expect(page.getByText("Showing 15 of 15 transactions")).toBeVisible();
    await expect(loadMoreBtn).not.toBeVisible();
  });
});
