import { render } from "@testing-library/svelte";
import { describe, expect, it } from "vitest";
import Amount from "./Amount.svelte";

/** The amount's own element, around the text and its smaller cents. */
const shown = (container: HTMLElement) => container.querySelector("span") as HTMLElement;

describe("Amount", () => {
  it("shows the cents smaller than the rest", () => {
    const { container } = render(Amount, { cents: 123450, currency: "EUR" });
    expect(shown(container).textContent).toBe("€1,234.50");
    expect(shown(container).querySelector("span")?.textContent).toBe(".50");
  });

  it("has no color unless asked", () => {
    const { container } = render(Amount, { cents: -500, currency: "EUR" });
    expect(shown(container).className).not.toMatch(/text-(positive|negative)/);
  });

  it("colors and signs a balance", () => {
    const owed = render(Amount, { cents: 500, currency: "EUR", tone: "balance" });
    expect(shown(owed.container).textContent).toBe("+€5");
    expect(shown(owed.container).className).toContain("text-positive");

    const owes = render(Amount, { cents: -500, currency: "EUR", tone: "balance" });
    expect(shown(owes.container).textContent).toBe("-€5");
    expect(shown(owes.container).className).toContain("text-negative");

    const settled = render(Amount, { cents: 0, currency: "EUR", tone: "balance" });
    expect(shown(settled.container).className).not.toMatch(/text-(positive|negative)/);
  });

  it("takes the caller's classes", () => {
    const { container } = render(Amount, { cents: 100, currency: "EUR", class: "text-lg" });
    expect(shown(container).className).toContain("text-lg");
  });
});
