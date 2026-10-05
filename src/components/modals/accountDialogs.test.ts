import { fireEvent, render, screen } from "@testing-library/svelte";
import { toast } from "svelte-sonner";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { t } from "@/lib/i18n/index.svelte";
import { dialogs } from "@/lib/state/dialogs.svelte";
import { session } from "@/lib/state/session.svelte";
import { api } from "@/services/api";
import { account } from "@/test/fixtures";
import type { PasswordStrength } from "@/types";
import ChangePasswordDialog from "./ChangePasswordDialog.svelte";
import RecoveryKeyDialog from "./RecoveryKeyDialog.svelte";

vi.mock("@/services/api", () => ({
  api: { changePassword: vi.fn(), passwordStrength: vi.fn(), replaceRecoveryKey: vi.fn() },
}));
vi.mock("svelte-sonner", () => ({ toast: { success: vi.fn(), info: vi.fn() } }));

const strong: PasswordStrength = { score: 4, acceptable: true, warning: null, suggestions: [] };

async function type(field: HTMLElement, value: string) {
  await fireEvent.input(field, { target: { value } });
}

async function submit() {
  const form = (await screen.findByRole("dialog")).querySelector("form");
  if (!form) throw new Error("The dialog has no form");
  await fireEvent.submit(form);
}

beforeEach(() => {
  vi.clearAllMocks();
  session.account = account();
  dialogs.changePassword = false;
  vi.mocked(api.passwordStrength).mockResolvedValue(strong);
});

describe("ChangePasswordDialog", () => {
  async function fill(next: string, confirm: string) {
    await type(await screen.findByLabelText(t("password.current")), "correct horse");
    await type(screen.getByLabelText(t("auth.newPassword")), next);
    await type(screen.getByLabelText(t("auth.confirmNewPassword")), confirm);
  }

  it("rates the new password as it is typed, and only then lets it through", async () => {
    dialogs.changePassword = true;
    render(ChangePasswordDialog);
    const change = await screen.findByRole("button", { name: t("password.submit") });
    expect(change).toHaveProperty("disabled", true);
    await fill("juniper walrus lantern", "juniper walrus lantern");
    await vi.waitFor(() => expect(change).toHaveProperty("disabled", false));
    expect(api.passwordStrength).toHaveBeenCalledWith("juniper walrus lantern", "alice");
    expect(screen.getByText(t("strength.strong"))).toBeTruthy();
  });

  it("changes the password and closes", async () => {
    vi.mocked(api.changePassword).mockResolvedValue(undefined);
    dialogs.changePassword = true;
    render(ChangePasswordDialog);
    await fill("juniper walrus lantern", "juniper walrus lantern");
    await submit();
    expect(api.changePassword).toHaveBeenCalledWith("correct horse", "juniper walrus lantern");
    await vi.waitFor(() => expect(dialogs.changePassword).toBe(false));
    expect(toast.success).toHaveBeenCalledWith(t("password.changed"));
  });

  it("needs the new password typed twice the same", async () => {
    dialogs.changePassword = true;
    render(ChangePasswordDialog);
    await fill("juniper walrus lantern", "juniper walrus lantern!");
    await submit();
    const dialog = await screen.findByRole("dialog");
    expect(dialog.textContent).toContain(t("password.mismatch"));
    expect(api.changePassword).not.toHaveBeenCalled();
  });

  it("says when the current password is wrong", async () => {
    vi.mocked(api.changePassword).mockRejectedValue(new Error("Your current password is wrong"));
    dialogs.changePassword = true;
    render(ChangePasswordDialog);
    await fill("juniper walrus lantern", "juniper walrus lantern");
    await submit();
    const dialog = await screen.findByRole("dialog");
    await vi.waitFor(() => expect(dialog.textContent).toContain("Your current password is wrong"));
    expect(dialogs.changePassword).toBe(true);
  });
});

describe("RecoveryKeyDialog", () => {
  const KEY = "AAAA-BBBB-CCCC-DDDD";

  it("shows a new account's key, and closes only once it is said to be saved", async () => {
    const onClose = vi.fn();
    render(RecoveryKeyDialog, { open: true, onClose, reason: "signup", recoveryKey: KEY });
    const dialog = await screen.findByRole("dialog");
    expect(dialog.textContent).toContain(t("recovery.signup"));
    expect(screen.getByLabelText(t("auth.recoveryKey")).textContent?.trim()).toBe(KEY);
    const done = screen.getByRole("button", { name: t("common.done") });
    expect(done).toHaveProperty("disabled", true);
    await fireEvent.click(screen.getByRole("checkbox", { name: t("recovery.saved") }));
    expect(done).toHaveProperty("disabled", false);
    await fireEvent.click(done);
    expect(onClose).toHaveBeenCalled();
  });

  it("copies the key", async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, "clipboard", { value: { writeText }, configurable: true });
    render(RecoveryKeyDialog, { open: true, onClose: vi.fn(), reason: "signup", recoveryKey: KEY });
    await fireEvent.click(await screen.findByRole("button", { name: t("recovery.copy") }));
    expect(writeText).toHaveBeenCalledWith(KEY);
    await vi.waitFor(() => expect(toast.success).toHaveBeenCalledWith(t("recovery.copied")));
  });

  it("asks for the password before making a new key, then shows it", async () => {
    vi.mocked(api.replaceRecoveryKey).mockResolvedValue(KEY);
    render(RecoveryKeyDialog, { open: true, onClose: vi.fn(), reason: "replace" });
    await type(await screen.findByLabelText(t("common.password")), "correct horse");
    await submit();
    expect(api.replaceRecoveryKey).toHaveBeenCalledWith("correct horse");
    const key = await screen.findByLabelText(t("auth.recoveryKey"));
    expect(key.textContent?.trim()).toBe(KEY);
    expect((await screen.findByRole("dialog")).textContent).toContain(t("recovery.replace"));
  });

  it("says when the password is wrong, and can be cancelled before a key is shown", async () => {
    vi.mocked(api.replaceRecoveryKey).mockRejectedValue(new Error("Wrong password"));
    const onClose = vi.fn();
    render(RecoveryKeyDialog, { open: true, onClose, reason: "replace" });
    await type(await screen.findByLabelText(t("common.password")), "nope");
    await submit();
    const dialog = await screen.findByRole("dialog");
    await vi.waitFor(() => expect(dialog.textContent).toContain("Wrong password"));
    await fireEvent.click(screen.getByRole("button", { name: t("common.cancel") }));
    expect(onClose).toHaveBeenCalled();
  });
});
