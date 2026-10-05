import { fireEvent, render, screen } from "@testing-library/svelte";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { t } from "@/lib/i18n/index.svelte";
import { askConfirm, confirmation } from "@/lib/state/confirm.svelte";
import BottomBar from "./BottomBar.svelte";
import ConfirmDialog from "./ConfirmDialog.svelte";
import QrScanOverlay from "./QrScanOverlay.svelte";

beforeEach(() => confirmation.settle(false));

describe("ConfirmDialog", () => {
  it("shows nothing until something is asked", () => {
    render(ConfirmDialog);
    expect(screen.queryByRole("alertdialog")).toBeNull();
  });

  it("asks the question, and answers yes when confirmed", async () => {
    render(ConfirmDialog);
    const answer = askConfirm({
      title: "Leave Trip?",
      description: "You can join again with an invite.",
      confirmLabel: "Leave",
      destructive: true,
    });
    const dialog = await screen.findByRole("alertdialog");
    expect(dialog.textContent).toContain("Leave Trip?");
    expect(dialog.textContent).toContain("You can join again with an invite.");
    await fireEvent.click(screen.getByRole("button", { name: "Leave" }));
    await expect(answer).resolves.toBe(true);
  });

  it("answers no when cancelled, with the usual words when none are given", async () => {
    render(ConfirmDialog);
    const answer = askConfirm({ title: "Stop repeating Rent?" });
    await screen.findByRole("alertdialog");
    expect(screen.getByRole("button", { name: t("common.confirm") })).toBeTruthy();
    await fireEvent.click(screen.getByRole("button", { name: t("common.cancel") }));
    await expect(answer).resolves.toBe(false);
  });
});

describe("BottomBar", () => {
  it("leads back to the group list", async () => {
    const onHome = vi.fn();
    render(BottomBar, { onHome });
    const home = screen.getByRole("button", { name: t("app.groups") });
    expect(home.hasAttribute("aria-current")).toBe(false);
    await fireEvent.click(home);
    expect(onHome).toHaveBeenCalled();
  });

  it("marks the group list as the page shown", () => {
    render(BottomBar, { onHome: vi.fn(), home: true });
    const home = screen.getByRole("button", { name: t("app.groups") });
    expect(home.getAttribute("aria-current")).toBe("page");
    expect(screen.getByRole("navigation", { name: t("app.nav") })).toBeTruthy();
  });
});

describe("QrScanOverlay", () => {
  it("hides the app while the camera scans, and shows it again after", () => {
    const { unmount } = render(QrScanOverlay, { onCancel: vi.fn() });
    expect(document.documentElement.classList.contains("scanning")).toBe(true);
    expect(screen.getByRole("region", { name: t("join.scanLabel") }).parentElement).toBe(
      document.body
    );
    unmount();
    expect(document.documentElement.classList.contains("scanning")).toBe(false);
    expect(screen.queryByRole("region", { name: t("join.scanLabel") })).toBeNull();
  });

  it("has a way out", async () => {
    const onCancel = vi.fn();
    render(QrScanOverlay, { onCancel });
    await fireEvent.click(screen.getByRole("button", { name: t("common.cancel") }));
    expect(onCancel).toHaveBeenCalled();
  });
});
