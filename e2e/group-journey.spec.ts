import { expect, test } from "@playwright/test";
import { installTauriMock } from "./fixtures/tauri-mock";

test.beforeEach(async ({ page }) => {
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
    await page.getByRole("button", { name: "+ Create Group" }).click();
    await expect(page.getByRole("heading", { name: "Create New Group" })).toBeVisible();

    // Fill form
    await page.locator("#input-group-name").fill("Rome Holiday");
    await page.locator("#select-group-currency").selectOption("EUR");

    const participantInputs = page.locator("input[placeholder^='Participant']");
    await participantInputs.nth(0).fill("Alice");
    await participantInputs.nth(1).fill("Bob");
    await participantInputs.nth(2).fill("Charlie");

    // Submit form
    await page.getByRole("button", { name: "Create Group", exact: true }).click();

    // Should immediately navigate into the newly created group workspace
    await expect(page.getByRole("heading", { name: "Rome Holiday" })).toBeVisible();
    await expect(page.getByText("Alice")).toBeVisible();
    await expect(page.getByText("Bob")).toBeVisible();
    await expect(page.getByText("Charlie")).toBeVisible();
    // Reimburse button should be hidden when there are 0 expenses / 0 balance
    await expect(page.getByRole("button", { name: "Reimburse" })).not.toBeVisible();

    // Navigate back to the dashboard
    await page.getByRole("button", { name: "← Back to All Groups" }).click();

    // Verify we are back on the dashboard with the group card visible
    await expect(page.getByRole("heading", { name: "Your Groups" })).toBeVisible();
    const groupCard = page.getByRole("button", { name: /Rome Holiday/ });
    await expect(groupCard).toBeVisible();

    // Click the group card (THIS WAS THE PREVIOUS BUG: clicking the card did nothing!)
    await groupCard.click();

    // Verify workspace successfully opens with full group details
    await expect(page.getByRole("heading", { name: "Rome Holiday" })).toBeVisible();
    await expect(page.getByText("Alice")).toBeVisible();
    await expect(page.getByText("Bob")).toBeVisible();
    await expect(page.getByText("Charlie")).toBeVisible();
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
    await page.getByRole("button", { name: "+ Create Group" }).click();
    await page.locator("#input-group-name").fill("Ski Trip 2026");
    await page.locator("#select-group-currency").selectOption("EUR");

    const participantInputs = page.locator("input[placeholder^='Participant']");
    await participantInputs.nth(0).fill("Alice");
    await participantInputs.nth(1).fill("Bob");
    // Remove 3rd participant
    await page.locator("button[title='Remove']").last().click();

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

    // Verify expense appears in list
    const expenseCard = page.locator(".space-y-2\\.5 > div").first();
    await expect(expenseCard).toBeVisible();
    await expect(expenseCard.getByRole("heading", { name: "Chalet Rental" })).toBeVisible();
    await expect(expenseCard.getByText("200.00 €")).toBeVisible();
    await expect(expenseCard.getByText("Paid by Alice", { exact: false })).toBeVisible();

    // Edit the expense
    await expenseCard.getByTitle("Edit expense").click();
    await expect(page.getByRole("heading", { name: "Edit Expense" })).toBeVisible();

    await page.locator("#input-expense-title").fill("Chalet Rental & Firewood");
    await page.locator("#input-expense-amount").fill("250.00");
    await page.getByRole("button", { name: "Save Changes" }).click();

    // Verify updated details
    await expect(expenseCard.getByRole("heading", { name: "Chalet Rental & Firewood" })).toBeVisible();
    await expect(expenseCard.getByText("250.00 €")).toBeVisible();

    // Verify "Edited (1)" badge is visible and clickable
    const editedBadge = expenseCard.getByRole("button", { name: /Edited/i });
    await expect(editedBadge).toBeVisible();
    await editedBadge.click();

    // Verify history modal
    await expect(page.getByRole("heading", { name: "Expense Revision History" })).toBeVisible();
    const historyModal = page.locator(".fixed");
    await expect(historyModal.getByText("Chalet Rental", { exact: true })).toBeVisible();
    await expect(historyModal.getByText("200.00 €")).toBeVisible();

    // Close history modal
    await page.getByRole("button", { name: "Close" }).click();

    // Check Balances tab
    await page.getByRole("button", { name: "Balances" }).click();
    await expect(page.getByText("Gets back")).toBeVisible();
    await expect(page.getByText("Owes")).toBeVisible();
    await expect(page.getByText("-125.00 €")).toBeVisible();

    // Check Settle Up tab
    await page.getByRole("button", { name: "Settle Up" }).click();
    await expect(page.getByText("Bob pays Alice")).toBeVisible();
    await expect(page.getByText("125.00 €")).toBeVisible();
    await expect(page.getByRole("button", { name: "Mark as Paid" })).toBeVisible();
  });
});
