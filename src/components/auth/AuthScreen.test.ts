import { fireEvent, render, screen } from "@testing-library/svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { i18n, t } from "@/lib/i18n/index.svelte";
import { api } from "@/services/api";
import { account } from "@/test/fixtures";
import AuthScreen from "./AuthScreen.svelte";

// What the device can do, as `native.svelte` reports it.
const device = vi.hoisted(() => ({ features: { share: false, scan: false, save: false } }));

vi.mock("@/services/api", () => ({
  api: { logIn: vi.fn(), signUp: vi.fn(), passwordStrength: vi.fn() },
}));
vi.mock("@/services/native.svelte", () => ({
  nativeFeatures: device.features,
  ScanCancelled: class extends Error {},
  scanQrCode: vi.fn(),
  closeTopLayer: () => false,
}));
vi.mock("svelte-sonner", () => ({ toast: { warning: vi.fn() } }));

async function type(field: HTMLElement, value: string) {
  await fireEvent.input(field, { target: { value } });
}

const submitButton = (label: string) => screen.getByRole("button", { name: label });

beforeEach(() => {
  vi.clearAllMocks();
  device.features.scan = false;
  i18n.choose("en");
  vi.mocked(api.passwordStrength).mockResolvedValue({
    score: 4,
    acceptable: true,
    warning: null,
    suggestions: [],
  });
});

afterEach(() => {
  localStorage.clear();
  i18n.choose("system");
});

describe("AuthScreen", () => {
  it("logs in with the username and password typed", async () => {
    const onAuthenticated = vi.fn();
    vi.mocked(api.logIn).mockResolvedValue(account());
    const { container } = render(AuthScreen, { onAuthenticated });
    expect(screen.getByRole("heading", { level: 1 }).textContent).toBe(t("auth.loginTitle"));
    await type(screen.getByLabelText(t("auth.username")), "alice");
    await type(screen.getByLabelText(t("common.password")), "correct horse");
    const form = container.querySelector("form");
    if (form) await fireEvent.submit(form);
    await vi.waitFor(() => expect(onAuthenticated).toHaveBeenCalled());
    expect(vi.mocked(api.logIn).mock.calls[0].slice(1)).toEqual(["alice", "correct horse"]);
  });

  it("shows why logging in failed", async () => {
    vi.mocked(api.logIn).mockRejectedValue(new Error("Wrong username or password"));
    const { container } = render(AuthScreen, { onAuthenticated: vi.fn() });
    await type(screen.getByLabelText(t("auth.username")), "alice");
    await type(screen.getByLabelText(t("common.password")), "nope");
    const form = container.querySelector("form");
    if (form) await fireEvent.submit(form);
    await vi.waitFor(() => expect(container.textContent).toContain("Wrong username or password"));
  });

  it("asks a new account for its password twice, once it is strong enough", async () => {
    render(AuthScreen, { onAuthenticated: vi.fn() });
    await fireEvent.click(screen.getByRole("tab", { name: t("auth.signUpTab") }));
    expect(screen.getByRole("heading", { level: 1 }).textContent).toBe(t("auth.signupTitle"));
    const create = submitButton(t("auth.createAccount"));
    expect(create).toHaveProperty("disabled", true);
    await type(screen.getByLabelText(t("auth.username")), "alice");
    await type(screen.getByLabelText(t("common.password")), "juniper walrus lantern");
    expect(screen.getByLabelText(t("auth.confirmPassword"))).toBeTruthy();
    await vi.waitFor(() => expect(create).toHaveProperty("disabled", false));
    expect(screen.getByText(t("strength.strong"))).toBeTruthy();
  });

  it("leads to the recovery key from a forgotten password, and back", async () => {
    render(AuthScreen, { onAuthenticated: vi.fn() });
    await fireEvent.click(screen.getByRole("button", { name: t("auth.forgot") }));
    expect(screen.getByLabelText(t("auth.recoveryKey"))).toBeTruthy();
    expect(screen.getByLabelText(t("auth.newPassword"))).toBeTruthy();
    expect(screen.queryByRole("tab", { name: t("auth.signUpTab") })).toBeNull();
    await fireEvent.click(screen.getByRole("button", { name: t("auth.backToLogin") }));
    expect(screen.queryByLabelText(t("auth.recoveryKey"))).toBeNull();
  });

  it("keeps the relay out of the way until asked for", async () => {
    render(AuthScreen, { onAuthenticated: vi.fn() });
    expect(screen.queryByLabelText(t("common.server"))).toBeNull();
    await fireEvent.click(screen.getByRole("button", { name: t("auth.changeServer") }));
    expect(screen.getByLabelText(t("common.server"))).toBeTruthy();
  });

  it("offers to scan another device's code only where the device can", () => {
    const without = render(AuthScreen, { onAuthenticated: vi.fn() });
    expect(screen.queryByRole("button", { name: t("auth.scan") })).toBeNull();
    without.unmount();
    device.features.scan = true;
    render(AuthScreen, { onAuthenticated: vi.fn() });
    expect(screen.getByRole("button", { name: t("auth.scan") })).toBeTruthy();
  });

  it("speaks the language picked at its bottom", async () => {
    render(AuthScreen, { onAuthenticated: vi.fn() });
    const english = t("auth.loginTitle");
    await fireEvent.click(screen.getByRole("button", { name: "Français" }));
    expect(i18n.language).toBe("fr");
    expect(screen.getByRole("heading", { level: 1 }).textContent).toBe(t("auth.loginTitle"));
    expect(t("auth.loginTitle")).not.toBe(english);
  });
});
