import { expect, type Page, test } from "@playwright/test";
import { installTauriMock } from "./fixtures/tauri-mock";

const now = new Date().toISOString();
const members = [
  { id: "p-alice", name: "Alice" },
  { id: "p-bob", name: "Bob" },
  { id: "p-carol", name: "Carol" },
];

/** A group where nobody owes anything. */
const settled = { id: "g-flat", name: "Flat", currency: "EUR", created_at: now, expenses: [] };
/** A group where Bob and Carol owe Alice. */
const owing = {
  id: "g-trip",
  name: "Lisbon Trip",
  currency: "EUR",
  created_at: now,
  expenses: [
    {
      id: "e-dinner",
      group_id: "g-trip",
      title: "Dinner",
      amount_cents: 9000,
      paid_by: "p-alice",
      splits: members.map((m) => ({ participant_id: m.id, shares: 1 })),
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

async function seed(page: Page, data: Record<string, unknown>) {
  await page.addInitScript((d) => Object.assign(window, d), data);
  await page.goto("/");
}

async function groupMenu(page: Page, item: string) {
  await page.getByRole("button", { name: "Group options" }).click();
  await page.getByRole("menuitem", { name: item }).click();
}

test("archiving puts a group away, and it can come back", async ({ page }) => {
  await seed(page, {
    __SEED_GROUPS__: [
      { ...settled, participants: members },
      { ...owing, participants: members },
    ],
  });
  await expect(page.getByText("2 active groups")).toBeVisible();

  await page.getByRole("button", { name: /Lisbon Trip/ }).click();
  await groupMenu(page, "Archive group");
  await expect(page.getByText('"Lisbon Trip" is archived')).toBeVisible();

  // Back on the list: one active group, the other folded under "Archived".
  await expect(page.getByText("1 active group")).toBeVisible();
  await expect(page.getByRole("button", { name: /Lisbon Trip/ })).not.toBeVisible();
  await page.getByText("Archived (1)").click();
  await page.getByRole("button", { name: /Lisbon Trip/ }).click();
  await expect(page.getByText("Archived", { exact: true })).toBeVisible();

  await groupMenu(page, "Unarchive group");
  await expect(page.getByText('"Lisbon Trip" is back in your groups')).toBeVisible();
  await page.goBack();
  await expect(page.getByText("2 active groups")).toBeVisible();
  await expect(page.getByText(/Archived \(/)).toHaveCount(0);
});

test("a settled group is deleted for everyone at once", async ({ page }) => {
  await seed(page, { __SEED_GROUPS__: [{ ...settled, participants: members }] });
  await page.getByRole("button", { name: /Flat/ }).click();
  await groupMenu(page, "Delete group…");

  const confirm = page.getByRole("alertdialog", { name: 'Delete "Flat" for everyone?' });
  await confirm.getByRole("button", { name: "Cancel" }).click();
  await expect(page.getByRole("heading", { name: "Flat", exact: true })).toBeVisible();

  await groupMenu(page, "Delete group…");
  await confirm.getByRole("button", { name: "Delete for everyone" }).click();
  await expect(page.getByText('"Flat" was deleted')).toBeVisible();
  await expect(page.getByText("No groups yet")).toBeVisible();
});

test("with balances to settle, deleting takes everyone's agreement", async ({ page }) => {
  // Bob already asked; the user is Alice.
  await seed(page, {
    __SEED_GROUPS__: [{ ...owing, participants: members, deletion_votes: ["p-bob"] }],
  });
  await page.getByRole("button", { name: /Lisbon Trip/ }).click();

  const request = page.getByRole("alert").filter({ hasText: "wants to delete this group" });
  await expect(request).toContainText("Bob wants to delete this group");
  await expect(request).toContainText("Still waiting for Alice and Carol.");

  // Refusing drops the request.
  await request.getByRole("button", { name: "Refuse" }).click();
  await expect(request).toHaveCount(0);

  // Asking again: it only records the user's agreement, since the others haven't agreed.
  await groupMenu(page, "Delete group…");
  const ask = page.getByRole("alertdialog", { name: 'Ask to delete "Lisbon Trip"?' });
  await expect(ask).toContainText("every member has to agree");
  await ask.getByRole("button", { name: "Ask to delete" }).click();
  await expect(request).toContainText("Alice wants to delete this group");
  await expect(request).toContainText("Still waiting for Bob and Carol.");
  await expect(page.getByRole("heading", { name: "Lisbon Trip", exact: true })).toBeVisible();

  await request.getByRole("button", { name: "Take back my agreement" }).click();
  await expect(request).toHaveCount(0);
});

test("the last agreement deletes the group", async ({ page }) => {
  await seed(page, {
    __SEED_GROUPS__: [{ ...owing, participants: members, deletion_votes: ["p-bob", "p-carol"] }],
  });
  await page.getByRole("button", { name: /Lisbon Trip/ }).click();
  const request = page.getByRole("alert").filter({ hasText: "want to delete this group" });
  await expect(request).toContainText("Bob and Carol want to delete this group");
  await request.getByRole("button", { name: "Agree to delete" }).click();
  await page
    .getByRole("alertdialog", { name: 'Delete "Lisbon Trip" for everyone?' })
    .getByRole("button", { name: "Delete for everyone" })
    .click();
  await expect(page.getByText('"Lisbon Trip" was deleted')).toBeVisible();
  await expect(page.getByText("No groups yet")).toBeVisible();
});

test("asking to delete needs to know who the user is", async ({ page }) => {
  await seed(page, {
    __SEED_GROUPS__: [{ ...owing, participants: members }],
    __SEED_IDENTITIES__: {},
  });
  await page.getByRole("button", { name: /Lisbon Trip/ }).click();
  await page.keyboard.press("Escape");
  await groupMenu(page, "Delete group…");
  await expect(page.getByText("Say who you are in this group before asking")).toBeVisible();
  await expect(page.getByRole("dialog", { name: /Who are you/ })).toBeVisible();
});

test("a dialog taller than the window scrolls", async ({ page }) => {
  await page.setViewportSize({ width: 700, height: 420 });
  await seed(page, { __SEED_GROUPS__: [{ ...settled, participants: members }] });
  await page.getByRole("button", { name: "Menu", exact: true }).click();
  await page.getByRole("menuitem", { name: /Account/ }).click();
  const dialog = page.getByRole("dialog", { name: "Account" });

  // It fits in the window, with its close button, and its end is reached by scrolling.
  const box = await dialog.boundingBox();
  expect(box && box.y >= 0 && box.y + box.height <= 420).toBe(true);
  await expect(dialog.getByRole("button", { name: "Close" })).toBeInViewport();
  const logOut = dialog.getByRole("button", { name: "Log out" });
  await expect(logOut).not.toBeInViewport();
  await logOut.scrollIntoViewIfNeeded();
  await expect(logOut).toBeInViewport();
});

test("a group a newer version of the app changed asks for an update", async ({ page }) => {
  await seed(page, {
    __SEED_GROUPS__: [
      { ...settled, participants: members },
      // As the core reads it: its name, and nothing else.
      {
        id: "g-ski",
        name: "Ski",
        currency: "EUR",
        created_at: now,
        participants: [],
        expenses: [],
        needs_update: true,
      },
    ],
    __SEED_IDENTITIES__: { "g-flat": "p-alice" },
  });
  // The account itself is fine: nothing asks to update the whole app.
  await expect(page.getByRole("button", { name: "Flat" })).toBeVisible();
  await expect(page.getByText("This version is too old for your account")).toHaveCount(0);

  const card = page.getByRole("listitem").filter({ hasText: "Ski" });
  await expect(card).toContainText("Update ezcount to open this group");
  await expect(card).not.toContainText("Total spent");

  await card.getByRole("button", { name: "Ski" }).click();
  await expect(page.getByRole("heading", { name: "Ski" })).toBeVisible();
  await expect(
    page.getByText("This group was changed by a newer version of ezcount")
  ).toBeVisible();
  // Nothing to read or change, and nobody is asked who they are in it.
  await expect(page.getByRole("tablist")).toHaveCount(0);
  await expect(page.getByRole("button", { name: "Add Expense" })).toHaveCount(0);
  await expect(page.getByRole("dialog")).toHaveCount(0);
});

test("an app too old for its account says so above the groups", async ({ page }) => {
  await seed(page, {
    __SEED_GROUPS__: [{ ...settled, participants: members }],
    __UPDATE_REQUIRED__: true,
  });
  await expect(page.getByRole("button", { name: "Flat" })).toBeVisible();
  const notice = page.getByTestId("update-notice");
  await expect(notice).toContainText("Update ezcount");
  await expect(notice).toContainText("This version is too old for your account");
  // An app is updated where it came from: only the web version offers to reload.
  await expect(notice.getByRole("button")).toHaveCount(0);
});
