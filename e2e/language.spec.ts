import { expect, type Page, test } from "@playwright/test";
import { installTauriMock } from "./fixtures/tauri-mock";

const now = new Date().toISOString();

const tripGroup = {
  id: "group-trip",
  name: "Lisbon Trip",
  currency: "EUR",
  created_at: now,
  participants: [
    { id: "p-alice", name: "Alice" },
    { id: "p-bob", name: "Bob" },
  ],
  expenses: [
    {
      id: "exp-flights",
      group_id: "group-trip",
      title: "Flights",
      amount_cents: 123450,
      paid_by: "p-alice",
      splits: [
        { participant_id: "p-alice", shares: 1 },
        { participant_id: "p-bob", shares: 1 },
      ],
      created_at: now,
      updated_at: now,
      history: [],
      is_reimbursement: false,
    },
    {
      id: "exp-payment",
      group_id: "group-trip",
      title: "Payment: Bob → Alice",
      amount_cents: 2000,
      paid_by: "p-bob",
      splits: [{ participant_id: "p-alice", shares: 1 }],
      created_at: now,
      updated_at: now,
      history: [],
      is_reimbursement: true,
    },
  ],
};

async function start(page: Page) {
  page.on("pageerror", (err) => console.error(">>> BROWSER ERROR:", err));
  await page.addInitScript(installTauriMock);
  await page.addInitScript((d) => Object.assign(window, d), { __SEED_GROUPS__: [tripGroup] });
  await page.goto("/");
}

test.describe("on a French device", () => {
  test.use({ locale: "fr-FR" });

  test("the app is in French, amounts and dates included", async ({ page }) => {
    await start(page);
    await expect(page.getByRole("heading", { name: "Vos groupes" })).toBeVisible();
    await expect(page.locator("html")).toHaveAttribute("lang", "fr");
    // Thousands are grouped and the currency follows the amount. The payment is not spending.
    await expect(page.getByText("1 234,50 €")).toBeVisible();

    await page.getByRole("button", { name: /Lisbon Trip/ }).click();
    await expect(page.getByRole("tab", { name: /Dépenses/ })).toBeVisible();
    await expect(page.getByText("Aujourd'hui")).toBeVisible();
    // The core titles payments in English: the app shows them in its language.
    await expect(page.getByRole("heading", { name: "Paiement : Bob → Alice" })).toBeVisible();

    await page.getByRole("tab", { name: "Soldes" }).click();
    const balances = page.getByRole("tabpanel");
    await expect(balances.getByText("+597,25 €")).toBeVisible();
    await expect(balances.getByText("-597,25 €").first()).toBeVisible();

    // English is a choice away, and stays chosen.
    await page.getByRole("button", { name: "Menu", exact: true }).click();
    await page.getByRole("menuitem", { name: /Compte/ }).click();
    await page.getByLabel("Langue").click();
    await page.getByRole("option", { name: "English" }).click();
    await page.keyboard.press("Escape");
    await expect(page.getByRole("tab", { name: "Balances" })).toBeVisible();
    await expect(balances.getByText("+€597.25")).toBeVisible();
    await page.reload();
    await expect(page.getByRole("tab", { name: "Balances" })).toBeVisible();
  });

  test("messages from the core are translated", async ({ page }) => {
    await start(page);
    await page.getByRole("button", { name: "Rejoindre avec un code" }).click();
    const dialog = page.getByRole("dialog");
    await dialog.getByLabel("Lien d'invitation").fill("nothing like an invite");
    await dialog.getByRole("button", { name: "Rejoindre le groupe" }).click();
    await expect(dialog.getByText("Ce n'est pas une invitation ezcount valide")).toBeVisible();
  });
});

test("the language can be chosen from an English device", async ({ page }) => {
  await start(page);
  await expect(page.getByRole("heading", { name: "Your Groups" })).toBeVisible();
  await page.getByRole("button", { name: "Menu", exact: true }).click();
  await page.getByRole("menuitem", { name: /Account/ }).click();
  await page.getByLabel("Language").click();
  await page.getByRole("option", { name: "Français" }).click();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("heading", { name: "Vos groupes" })).toBeVisible();
  await expect(page.locator("html")).toHaveAttribute("lang", "fr");
});
