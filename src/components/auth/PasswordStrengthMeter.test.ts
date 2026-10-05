import { render, screen } from "@testing-library/svelte";
import { afterEach, describe, expect, it } from "vitest";
import { i18n } from "@/lib/i18n/index.svelte";
import type { PasswordStrength } from "@/types";
import PasswordStrengthMeter from "./PasswordStrengthMeter.svelte";

const strength = (more: Partial<PasswordStrength>): PasswordStrength => ({
  score: 0,
  acceptable: false,
  warning: null,
  suggestions: [],
  ...more,
});

/** How many of the bar's four parts are lit. */
const lit = (container: HTMLElement) =>
  container.querySelectorAll(".bg-negative, .bg-warning, .bg-positive").length;

afterEach(() => i18n.choose("en"));

describe("PasswordStrengthMeter", () => {
  it("gives a verdict the password field can be described by", () => {
    render(PasswordStrengthMeter, { strength: strength({ score: 1 }), id: "password-strength" });
    const verdict = screen.getByText("Too weak").closest("p");
    expect(verdict?.id).toBe("password-strength");
  });

  it("lights one part per point, and one even for nothing", () => {
    expect(lit(render(PasswordStrengthMeter, { strength: strength({}), id: "a" }).container)).toBe(
      1
    );
    const good = strength({ score: 3, acceptable: true });
    expect(lit(render(PasswordStrengthMeter, { strength: good, id: "b" }).container)).toBe(3);
  });

  it("says what to change while the password is too weak", () => {
    const weak = strength({ score: 2, warning: "This is a top-10 common password." });
    const { container } = render(PasswordStrengthMeter, { strength: weak, id: "a" });
    expect(container.textContent).toContain("Weak");
    expect(container.textContent).toContain("This is a top-10 common password.");
  });

  it("falls back to the first suggestion without a warning", () => {
    const weak = strength({ score: 2, suggestions: ["Add another word or two.", "More"] });
    const { container } = render(PasswordStrengthMeter, { strength: weak, id: "a" });
    expect(container.textContent).toContain("Add another word or two.");
    expect(container.textContent).not.toContain("More");
  });

  it("says a strong password is only too short", () => {
    const short = strength({ score: 4, acceptable: false });
    const { container } = render(PasswordStrengthMeter, { strength: short, id: "a" });
    expect(container.textContent).toContain("Too short");
    expect(container.querySelector(".bg-warning")).not.toBeNull();
  });

  it("gives no advice once the password is accepted", () => {
    const strong = strength({ score: 4, acceptable: true, suggestions: ["Ignored"] });
    const { container } = render(PasswordStrengthMeter, { strength: strong, id: "a" });
    expect(container.textContent).toContain("Strong");
    expect(container.textContent).not.toContain("Ignored");
  });

  it("speaks the app's language", () => {
    i18n.choose("fr");
    const { container } = render(PasswordStrengthMeter, { strength: strength({}), id: "a" });
    expect(container.textContent).toContain("Trop faible");
  });
});
