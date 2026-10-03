import { expect, type Page, test } from "@playwright/test";
import { installTauriMock } from "./fixtures/tauri-mock";

const now = new Date().toISOString();

const group = (id: string, name: string) => ({
  id,
  name,
  currency: "EUR",
  created_at: now,
  participants: [
    { id: `${id}-alice`, name: "Alice" },
    { id: `${id}-bob`, name: "Bob" },
  ],
  expenses: [],
});

// A 1×1 PNG: the app shrinks whatever it is given into its own small picture.
const PNG = Buffer.from(
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==",
  "base64"
);
const picture = { name: "me.png", mimeType: "image/png", buffer: PNG };

test.beforeEach(async ({ page }) => {
  page.on("pageerror", (err) => console.error(">>> BROWSER ERROR:", err));
  await page.addInitScript(installTauriMock);
});

async function seed(page: Page, data: Record<string, unknown>) {
  await page.addInitScript((d) => Object.assign(window, d), data);
}

async function openAccount(page: Page) {
  await page.getByRole("button", { name: "Menu", exact: true }).click();
  await page.getByRole("menuitem", { name: /Account/ }).click();
  return page.getByRole("dialog", { name: "Account" });
}

test("a profile name and picture show in every group", async ({ page }) => {
  await seed(page, { __SEED_GROUPS__: [group("trip", "Lisbon Trip"), group("flat", "Flat")] });
  await page.goto("/");

  const dialog = await openAccount(page);
  // Nothing to save until something changes.
  await expect(dialog.getByRole("button", { name: "Save profile" })).toHaveCount(0);
  await dialog.getByLabel("Name", { exact: true }).fill("Alice Martin");
  await dialog.getByLabel("Profile picture").setInputFiles(picture);
  await expect(dialog.getByTestId("picture-preview")).toHaveAttribute("src", /^data:image\//);
  await dialog.getByRole("button", { name: "Save profile" }).click();
  await expect(page.getByText("Profile saved")).toBeVisible();
  await page.keyboard.press("Escape");

  // The menu names the user by their profile.
  await page.getByRole("button", { name: "Menu", exact: true }).click();
  await expect(page.getByRole("menuitem", { name: /Alice Martin/ })).toBeVisible();
  await page.keyboard.press("Escape");

  // Both groups show the new name and the picture on the user's member, and only there.
  for (const name of ["Lisbon Trip", "Flat"]) {
    await page.getByRole("button", { name: new RegExp(name) }).click();
    const me = page.getByRole("button", { name: "Rename or remove Alice Martin" });
    await expect(me.locator("img")).toHaveAttribute("src", /^data:image\//);
    await expect(page.getByRole("button", { name: "Rename or remove Bob" })).toBeVisible();
    await expect(
      page.getByRole("button", { name: "Rename or remove Bob" }).locator("img")
    ).toHaveCount(0);
    await page.goBack();
  }

  // Removing the picture takes it off the groups; the name stays.
  const again = await openAccount(page);
  await again.getByRole("button", { name: "Remove picture" }).click();
  await again.getByRole("button", { name: "Save profile" }).click();
  await expect(again.getByRole("button", { name: "Save profile" })).toHaveCount(0);
  await page.keyboard.press("Escape");
  await page.getByRole("button", { name: /Lisbon Trip/ }).click();
  const me = page.getByRole("button", { name: "Rename or remove Alice Martin" });
  await expect(me).toBeVisible();
  await expect(me.locator("img")).toHaveCount(0);
});

test("saying who you are gives that member your profile", async ({ page }) => {
  const trip = group("trip", "Lisbon Trip");
  await seed(page, {
    __SEED_GROUPS__: [trip],
    __SEED_IDENTITIES__: {},
    __PROFILE__: {
      display_name: "Bobby",
      avatar: `data:image/png;base64,${PNG.toString("base64")}`,
    },
  });
  await page.goto("/");
  await page.getByRole("button", { name: /Lisbon Trip/ }).click();

  await page.getByRole("dialog").getByRole("button", { name: "Bob", exact: true }).click();
  const me = page.getByRole("button", { name: "Rename or remove Bobby" });
  await expect(me.locator("img")).toBeVisible();
  await expect(page.getByRole("button", { name: "Rename or remove Alice" })).toBeVisible();
});

test("a file that isn't a picture is refused", async ({ page }) => {
  await seed(page, { __SEED_GROUPS__: [group("trip", "Lisbon Trip")] });
  await page.goto("/");
  const dialog = await openAccount(page);
  await dialog
    .getByLabel("Profile picture")
    .setInputFiles({ name: "notes.txt", mimeType: "text/plain", buffer: Buffer.from("hello") });
  await expect(dialog.getByText("This file isn't a picture the app can read.")).toBeVisible();
  await expect(dialog.getByRole("button", { name: "Save profile" })).toHaveCount(0);
});

test("a group gets a picture and a description", async ({ page }) => {
  await seed(page, { __SEED_GROUPS__: [group("trip", "Lisbon Trip")] });
  await page.goto("/");
  await page.getByRole("button", { name: /Lisbon Trip/ }).click();

  await page.getByRole("button", { name: "Group options" }).click();
  await page.getByRole("menuitem", { name: "Edit group" }).click();
  const dialog = page.getByRole("dialog", { name: "Edit Group" });
  await dialog.getByLabel("Group picture").setInputFiles(picture);
  await dialog.getByLabel("Description").fill("A week in Lisbon\n12 to 19 May");
  await dialog.getByRole("button", { name: "Save" }).click();
  await expect(dialog).not.toBeVisible();

  await expect(page.getByText("A week in Lisbon")).toBeVisible();
  await expect(page.getByTestId("group-picture")).toHaveAttribute("src", /^data:image\//);

  // The group list shows them too.
  await page.goBack();
  const card = page.getByRole("listitem").filter({ hasText: "Lisbon Trip" });
  await expect(card.getByTestId("group-picture")).toBeVisible();
  await expect(card.getByText("A week in Lisbon")).toBeVisible();

  // Both can be taken off again.
  await page.getByRole("button", { name: /Lisbon Trip/ }).click();
  await page.getByRole("button", { name: "Group options" }).click();
  await page.getByRole("menuitem", { name: "Edit group" }).click();
  await dialog.getByRole("button", { name: "Remove picture" }).click();
  await dialog.getByLabel("Description").fill("");
  await dialog.getByRole("button", { name: "Save" }).click();
  await expect(dialog).not.toBeVisible();
  await expect(page.getByTestId("group-picture")).toHaveCount(0);
  await expect(page.getByText("A week in Lisbon")).toHaveCount(0);
});
