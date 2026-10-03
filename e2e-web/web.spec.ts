import { type Browser, expect, type Page, test } from "@playwright/test";

// The web version against a real relay: no mock, so these cover the Rust core in the browser
// (storage in OPFS, sync, the Tauri stand-in in src/web/). Each browser context is a device.

const PASSWORD = "tangerine kayak mosaic";

/** Uncaught errors and console errors (Content-Security-Policy violations among them). */
let problems: string[] = [];

test.afterEach(() => {
  const found = problems;
  problems = [];
  expect(found).toEqual([]);
});

async function device(browser: Browser): Promise<Page> {
  const context = await browser.newContext();
  context.on("page", watch);
  const page = await context.newPage();
  return page;
}

function watch(page: Page) {
  page.on("pageerror", (err) => problems.push(`${page.url()}: ${err.message}`));
  page.on("console", (message) => {
    if (message.type() === "error") problems.push(`${page.url()}: ${message.text()}`);
  });
}

async function signUp(page: Page, username: string) {
  await page.goto("/");
  await page.getByRole("tab", { name: "Sign up" }).click();
  await page.getByLabel("Username").fill(username);
  await page.getByLabel("Password", { exact: true }).fill(PASSWORD);
  await page.getByLabel("Confirm password").fill(PASSWORD);
  await page.getByRole("button", { name: "Create Account" }).click();

  const dialog = page.getByRole("dialog", { name: "Save your recovery key" });
  await dialog.getByLabel("I've saved it somewhere safe").check();
  await dialog.getByRole("button", { name: "Done" }).click();
  await expect(page.getByText("No groups yet")).toBeVisible();
}

async function logIn(page: Page, username: string) {
  await page.goto("/");
  await page.getByLabel("Username").fill(username);
  await page.getByLabel("Password", { exact: true }).fill(PASSWORD);
  await page.getByRole("button", { name: "Log In" }).click();
}

async function addExpense(page: Page, title: string, amount: string) {
  await page.getByRole("button", { name: "Add Expense" }).click();
  const dialog = page.getByRole("dialog");
  await dialog.getByLabel("Description").fill(title);
  await dialog.getByLabel("Amount").fill(amount);
  await dialog.getByRole("button", { name: "Save Expense" }).click();
  await expect(dialog).not.toBeVisible();
}

test("keeps its data and syncs it with the user's other devices", async ({ browser }) => {
  const username = `alice${Date.now()}`;
  const laptop = await device(browser);
  await signUp(laptop, username);

  await laptop.getByRole("button", { name: "Create Group" }).click();
  await laptop.locator("#input-group-name").fill("Lisbon");
  await laptop.locator("input[placeholder^='Participant']").first().fill("Bob");
  await laptop.getByRole("button", { name: "Create Group", exact: true }).click();
  await expect(laptop.getByRole("heading", { name: "Lisbon" })).toBeVisible();
  await addExpense(laptop, "Dinner", "30");
  await expect(laptop.getByTestId("expense-item").filter({ hasText: "Dinner" })).toBeVisible();

  // Stored in the browser: still there after a reload, which reopens the group.
  await laptop.reload();
  await expect(laptop.getByRole("heading", { name: "Lisbon" })).toBeVisible();
  await expect(laptop.getByTestId("expense-item").filter({ hasText: "Dinner" })).toBeVisible();

  // Another device logs in and gets the group from the relay.
  const phone = await device(browser);
  await logIn(phone, username);
  await phone.getByRole("button", { name: /Lisbon/ }).click();
  await expect(phone.getByTestId("expense-item").filter({ hasText: "Dinner" })).toBeVisible();

  // Its edits reach the first device through background sync.
  await addExpense(phone, "Taxi", "12.50");
  await expect(laptop.getByTestId("expense-item").filter({ hasText: "Taxi" })).toBeVisible({
    timeout: 45_000,
  });
});

test("runs in one tab at a time", async ({ browser }) => {
  const first = await device(browser);
  await first.goto("/");
  await expect(first.getByRole("heading", { name: "Welcome back" })).toBeVisible();

  const second = await first.context().newPage();
  await second.goto("/");
  await expect(second.getByText("ezcount is already open in another tab")).toBeVisible();
});

async function inviteToNewGroup(page: Page, name: string): Promise<string> {
  await page.getByRole("button", { name: "Create Group" }).click();
  await page.locator("#input-group-name").fill(name);
  await page.locator("input[placeholder^='Participant']").first().fill("Bob");
  await page.getByRole("button", { name: "Create Group", exact: true }).click();
  await page.getByRole("button", { name: "Invite" }).click();
  const invite = await page.getByLabel("Invite link").inputValue();
  expect(invite).toMatch(/\/join#v=2&g=.+&k=.+/);
  await page.keyboard.press("Escape");
  return invite;
}

async function joinFromDialog(page: Page, group: string) {
  const join = page.getByRole("dialog", { name: "Join a Group" });
  await join.getByRole("button", { name: "Join Group" }).click();
  await expect(page.getByRole("heading", { name: group })).toBeVisible();
}

test("joins a group from an invite link, through the relay's join page", async ({ browser }) => {
  const alice = await device(browser);
  await signUp(alice, `alice${Date.now()}`);
  const invite = await inviteToNewGroup(alice, "Flat");

  const bob = await device(browser);
  await signUp(bob, `bob${Date.now()}`);
  await bob.close();
  const page = await bob.context().newPage();
  await page.goto(invite);
  await page.getByRole("link", { name: "Open in your browser" }).click();
  // The key leaves the address bar once read.
  await expect(page).toHaveURL(/127\.0\.0\.1:8787\/$/);
  await joinFromDialog(page, "Flat");
});

test("hands an invite opened in a new tab to the open one", async ({ browser }) => {
  const alice = await device(browser);
  await signUp(alice, `alice${Date.now()}`);
  const invite = await inviteToNewGroup(alice, "Ski");

  const bob = await device(browser);
  await signUp(bob, `bob${Date.now()}`);
  const tab = await bob.context().newPage();
  await tab.goto(`/${new URL(invite).hash}`);
  await expect(tab.getByText("the invite was sent there")).toBeVisible();
  await joinFromDialog(bob, "Ski");
});
