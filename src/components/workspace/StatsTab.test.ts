import { render, screen } from "@testing-library/svelte";
import { describe, expect, it } from "vitest";
import { t } from "@/lib/i18n/index.svelte";
import { equally, expense, group } from "@/test/fixtures";
import StatsTab from "./StatsTab.svelte";

const items = (testId: string) =>
  [...screen.getByTestId(testId).querySelectorAll(":scope > li")].map((li) =>
    (li.textContent ?? "").replace(/\s+/g, " ").trim()
  );

describe("StatsTab", () => {
  it("says there is nothing to show before the first expense", () => {
    const paid = expense({ is_reimbursement: true });
    const { container } = render(StatsTab, { group: group({ expenses: [paid] }) });
    expect(container.textContent).toContain(t("stats.empty"));
    expect(screen.queryByTestId("stats-categories")).toBeNull();
  });

  it("shows the total, the average, and what came in apart", () => {
    const trip = group({
      expenses: [
        expense({ id: "a", amount_cents: 3000 }),
        expense({ id: "b", amount_cents: 1000 }),
        expense({ id: "c", amount_cents: 700, income: true }),
      ],
    });
    const { container } = render(StatsTab, { group: trip });
    const text = (container.textContent ?? "").replace(/\s+/g, " ");
    expect(text).toContain(`${t("stats.total")} €40`);
    expect(text).toContain(`${t("stats.count")} 2`);
    expect(text).toContain(`${t("stats.average")} €20`);
    expect(text).toContain(`${t("stats.income")} €7`);
  });

  it("breaks the spending down by category, by person and by month", () => {
    const trip = group({
      expenses: [
        expense({
          id: "a",
          category: "food",
          amount_cents: 3000,
          splits: equally("alice", "bob"),
          created_at: new Date(2026, 2, 10, 12).toISOString(),
        }),
        expense({
          id: "b",
          amount_cents: 1000,
          paid_by: "bob",
          splits: equally("bob"),
          created_at: new Date(2026, 1, 10, 12).toISOString(),
        }),
      ],
    });
    render(StatsTab, { group: trip });
    const [food, none] = items("stats-categories");
    expect(food).toContain("Restaurants");
    expect(food).toContain("€30 · 75%");
    expect(none).toContain(t("category.none"));
    expect(none).toContain("€10 · 25%");

    const [bob, alice, carol] = items("stats-people");
    expect(bob).toContain("Bob €25 · 63%");
    expect(alice).toContain("Alice €15 · 38%");
    expect(carol).toContain("Carol €0 · 0%");

    expect(items("stats-months")).toEqual(["March 2026 €30", "February 2026 €10"]);
  });
});
