import { expect, type Page, test } from "@playwright/test";
import { installTauriMock, MOCK_PASSWORD, MOCK_SERVER } from "./fixtures/tauri-mock";

const created = "2026-01-05T12:00:00.000Z";
const group = (id: string, name: string) => ({
  id,
  name,
  currency: "EUR",
  created_at: created,
  participants: [
    { id: "p-alice", name: "Alice" },
    { id: "p-bob", name: "Bob" },
  ],
  expenses: [],
});
const flat = group("group-flat", "Flat");
const trip = group("group-trip", "Trip");

/** What a device shows to get something: a phone scans it. */
const code = (purpose: string) =>
  `ezcount://receive?server=${encodeURIComponent(MOCK_SERVER)}&code=abc&for=${purpose}`;

async function seed(page: Page, data: object) {
  page.on("pageerror", (err) => console.error(">>> BROWSER ERROR:", err));
  await page.addInitScript(installTauriMock);
  await page.addInitScript((d) => Object.assign(window, d), data);
  await page.goto("/");
}

test("a computer logs in by showing a code to a phone", async ({ page }) => {
  await seed(page, { __LOGGED_OUT__: true, __PHONE_SCANS__: 2 });
  await page.getByRole("button", { name: "Connect with your phone" }).click();
  const dialog = page.getByRole("dialog");
  await expect(dialog.getByRole("img", { name: "QR code for a phone to scan" })).toBeVisible();
  await expect(dialog.getByText("Waiting for the phone…")).toBeVisible();
  // The phone scans it: this device is in, without a password typed here.
  await expect(page.getByText("No groups yet")).toBeVisible({ timeout: 10_000 });
  await expect(dialog).not.toBeVisible();
});

test("a computer joins a group by showing a code to a phone that is in it", async ({ page }) => {
  await seed(page, { __SEED_GROUPS__: [flat], __REMOTE_GROUPS__: [trip], __PHONE_SCANS__: 1 });
  await page.getByRole("button", { name: "Join with code" }).click();
  const dialog = page.getByRole("dialog");
  await dialog.getByRole("button", { name: "Receive from a phone" }).click();
  await expect(dialog.getByRole("img", { name: "QR code for a phone to scan" })).toBeVisible();
  await expect(page.getByText('Joined "Trip"')).toBeVisible({ timeout: 10_000 });
  await expect(page.getByRole("heading", { name: "Trip" })).toBeVisible();
});

test("going back from the code leaves the link field", async ({ page }) => {
  await seed(page, { __SEED_GROUPS__: [flat] });
  await page.getByRole("button", { name: "Join with code" }).click();
  const dialog = page.getByRole("dialog");
  await dialog.getByRole("button", { name: "Receive from a phone" }).click();
  await dialog.getByRole("button", { name: "Back" }).click();
  await expect(dialog.getByLabel("Invite link")).toBeVisible();
});

test("a phone sends a group to the computer whose code it scans", async ({ page }) => {
  await seed(page, {
    __SEED_GROUPS__: [flat],
    __NATIVE__: { scan: true },
    __SCANNED__: code("group"),
  });
  await page.getByRole("button", { name: /Flat/ }).click();
  await page.getByRole("button", { name: "Invite" }).click();
  await page.getByRole("button", { name: "Send to a computer" }).click();
  await expect(page.getByText('Sent: the other device joins "Flat"')).toBeVisible();
  expect(await page.evaluate(() => (window as any).__sent)).toEqual([{ group: "group-flat" }]);
});

test("a phone logs in the device whose code it scans, with the password", async ({ page }) => {
  await seed(page, {
    __SEED_GROUPS__: [flat],
    __NATIVE__: { scan: true },
    __SCANNED__: code("login"),
  });
  await page.getByRole("button", { name: "Menu" }).click();
  await page.getByRole("menuitem", { name: "Connect a device" }).click();
  const dialog = page.getByRole("dialog");
  const scan = dialog.getByRole("button", { name: "Scan the other device's code" });
  await expect(scan).toBeDisabled();
  await dialog.getByLabel("Password").fill(MOCK_PASSWORD);
  await scan.click();
  await expect(page.getByText("The other device is being logged in")).toBeVisible();
  expect(await page.evaluate(() => (window as any).__sent)).toEqual([{ login: true }]);
});

test("a code shown to join a group doesn't take an account", async ({ page }) => {
  await seed(page, {
    __SEED_GROUPS__: [flat],
    __NATIVE__: { scan: true },
    __SCANNED__: code("group"),
  });
  await page.getByRole("button", { name: "Menu" }).click();
  await page.getByRole("menuitem", { name: "Connect a device" }).click();
  const dialog = page.getByRole("dialog");
  await dialog.getByLabel("Password").fill(MOCK_PASSWORD);
  await dialog.getByRole("button", { name: "Scan the other device's code" }).click();
  await expect(page.getByText("Could not log the other device in")).toBeVisible();
  await expect(page.getByText(/This code is for joining a group/)).toBeVisible();
});

test("computers aren't offered to scan", async ({ page }) => {
  await seed(page, { __SEED_GROUPS__: [flat] });
  await page.getByRole("button", { name: /Flat/ }).click();
  await page.getByRole("button", { name: "Invite" }).click();
  await expect(page.getByRole("button", { name: "Copy Link" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Send to a computer" })).toHaveCount(0);
});
