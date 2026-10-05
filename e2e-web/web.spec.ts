import { type Browser, expect, type Page, test } from "@playwright/test";

// The web version against a real relay: no mock, so these cover the Rust core in the browser
// (storage in OPFS, sync, the Tauri stand-in in src/web/). Each browser context is a device.

const PASSWORD = "tangerine kayak mosaic";

/** Uncaught errors and console errors (Content-Security-Policy violations among them). */
let problems: string[] = [];
/** What the browser logs for requests a test has the relay refuse on purpose. */
let refusals: RegExp | null = null;

test.afterEach(() => {
  const found = problems.filter((problem) => !refusals?.test(problem));
  problems = [];
  refusals = null;
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
  await dialog.getByLabel("Amount", { exact: true }).fill(amount);
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

  // Its edits reach the first device through background sync, which offers to show them.
  await addExpense(phone, "Taxi", "12.50");
  const refresh = laptop.getByRole("button", { name: /New changes from the group/ });
  await expect(refresh).toBeVisible({ timeout: 45_000 });
  await refresh.click();
  await expect(laptop.getByTestId("expense-item").filter({ hasText: "Taxi" })).toBeVisible();
});

test("shows a code to log a phone in, after the password", async ({ browser }) => {
  const laptop = await device(browser);
  await signUp(laptop, `carol${Date.now()}`);

  await laptop.getByRole("button", { name: "Menu", exact: true }).click();
  await laptop.getByRole("menuitem", { name: "Connect a device" }).click();
  const dialog = laptop.getByRole("dialog", { name: "Connect another device" });
  await dialog.getByLabel("Password").fill(PASSWORD);
  await dialog.getByRole("button", { name: "Show Code" }).click();
  // The relay took the link: it says how long it works.
  await expect(dialog.getByRole("img", { name: /QR code/ })).toBeVisible();
  await expect(dialog.getByText(/Works once, for another [12]:\d\d/)).toBeVisible();
  // The browser has no camera scanning, so it doesn't offer to log in that way.
  const phone = await device(browser);
  await phone.goto("/");
  await expect(phone.getByRole("button", { name: "Log In" })).toBeVisible();
  await expect(phone.getByRole("button", { name: "Scan a code to log in" })).toHaveCount(0);
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

test("deletes an account for good, as the relay's pages say", async ({ browser }) => {
  // A wrong password, then the account's document and its login once they are gone.
  refusals = /the server responded with a status of (401|410)/;
  // The privacy policy leads to how to delete an account, and that page to the web version.
  const reader = await device(browser);
  await reader.goto("/privacy");
  await expect(reader.getByRole("heading", { name: "Privacy policy" })).toBeVisible();
  await reader.getByRole("link", { name: "Français" }).click();
  await expect(reader.getByRole("heading", { name: "Politique de confidentialité" })).toBeVisible();
  await reader.getByRole("link", { name: "comment supprimer votre compte" }).click();
  await expect(
    reader.getByRole("heading", { name: "Supprimer votre compte ezcount" })
  ).toBeVisible();
  await expect(
    reader.getByRole("link", { name: "ouvrez ezcount dans votre navigateur" })
  ).toHaveAttribute("href", "./");

  const username = `dave${Date.now()}`;
  const laptop = await device(browser);
  await signUp(laptop, username);
  const phone = await device(browser);
  await logIn(phone, username);
  await expect(phone.getByText("No groups yet")).toBeVisible();

  await laptop.getByRole("button", { name: "Menu", exact: true }).click();
  await laptop.getByRole("menuitem", { name: /Account/ }).click();
  await laptop
    .getByRole("dialog", { name: "Account" })
    .getByRole("button", { name: "Delete account" })
    .click();
  const dialog = laptop.getByRole("dialog", { name: `Delete the account ${username}?` });
  await dialog.getByLabel("Password").fill("not my password");
  await dialog.getByRole("button", { name: "Delete Account" }).click();
  await expect(dialog.getByText("Wrong password")).toBeVisible();
  await dialog.getByLabel("Password").fill(PASSWORD);
  await dialog.getByRole("button", { name: "Delete Account" }).click();
  await expect(laptop.getByRole("heading", { name: "Welcome back" })).toBeVisible();

  // The other device finds out at its next sync, and nothing is left to log in to.
  await expect(phone.getByText("Your account was deleted on another device")).toBeVisible({
    timeout: 45_000,
  });
  await expect(phone.getByRole("heading", { name: "Welcome back" })).toBeVisible();
  await logIn(phone, username);
  await expect(phone.getByText("Wrong username or password")).toBeVisible();
});
