import { afterEach, describe, expect, it } from "vitest";
import { backendText, summaryText } from "./backend";
import { i18n } from "./index.svelte";

afterEach(() => i18n.choose("en"));

describe("backendText", () => {
  it("leaves the core's English as it is in English", () => {
    expect(backendText("Group not found")).toBe("Group not found");
  });

  it("translates a message it knows", () => {
    i18n.choose("fr");
    expect(backendText("Group not found")).toBe("Groupe introuvable");
    expect(backendText("This IBAN is not valid")).toBe("Cet IBAN n'est pas valide");
  });

  it("carries the values a message holds", () => {
    i18n.choose("fr");
    expect(backendText("The items add up to 12.00, not the expense's 15.00")).toBe(
      "Les articles totalisent 12.00, pas les 15.00 de la dépense"
    );
    expect(backendText("This message is too long (2000 characters at most)")).toBe(
      "Ce message est trop long (2000 caractères au plus)"
    );
    expect(backendText("This comment is too long (500 characters at most)")).toBe(
      "Ce commentaire est trop long (500 caractères au plus)"
    );
  });

  it("shows what it doesn't know in English", () => {
    i18n.choose("fr");
    expect(backendText("Something new went wrong")).toBe("Something new went wrong");
  });
});

describe("summaryText", () => {
  const summary = "Title changed from 'Taxi' to 'Cab'; Amount changed from 10.00 to 12.00";

  it("is the core's own in English", () => {
    expect(summaryText(summary)).toBe(summary);
  });

  it("translates each change of an edit", () => {
    i18n.choose("fr");
    expect(summaryText(summary)).toBe(
      "Titre changé de « Taxi » à « Cab » ; Montant changé de 10.00 à 12.00"
    );
  });

  it("names the categories an edit changed", () => {
    i18n.choose("fr");
    expect(summaryText("Category changed from none to food")).toBe(
      "Catégorie changée de « Sans catégorie » à « Restaurants »"
    );
  });
});
