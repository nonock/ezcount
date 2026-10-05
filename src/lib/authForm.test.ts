import { toast } from "svelte-sonner";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { api } from "@/services/api";
import { ScanCancelled, scanQrCode } from "@/services/native.svelte";
import { account } from "@/test/fixtures";
import { AuthForm } from "./authForm.svelte";
import { t } from "./i18n/index.svelte";
import { DEFAULT_SERVER, rememberedServer } from "./server";
import { dialogs } from "./state/dialogs.svelte";

vi.mock("@/services/api", () => ({
  api: { logIn: vi.fn(), signUp: vi.fn(), recoverAccount: vi.fn(), logInWithLink: vi.fn() },
}));
vi.mock("@/services/native.svelte", () => {
  class ScanCancelled extends Error {}
  return { ScanCancelled, scanQrCode: vi.fn() };
});
vi.mock("svelte-sonner", () => ({ toast: { warning: vi.fn() } }));

const RELAY = "https://relay.example.com";
const onAuthenticated = vi.fn();
let form: AuthForm;

beforeEach(() => {
  vi.clearAllMocks();
  form = new AuthForm(onAuthenticated);
  form.username = "alice";
  form.password = "correct horse";
});

afterEach(() => localStorage.clear());

describe("the form", () => {
  it("starts on the login, at the relay last used", () => {
    expect(form.mode).toBe("login");
    expect(form.newPassword).toBe(false);
    expect(form.serverUrl).toBe(DEFAULT_SERVER);
  });

  it("forgets the password and the error when going to another mode", () => {
    form.confirmPassword = "correct horse";
    form.error = "Wrong password";
    form.switchMode("signup");
    expect(form.newPassword).toBe(true);
    expect(form.password).toBe("");
    expect(form.confirmPassword).toBe("");
    expect(form.error).toBeNull();
    expect(form.username).toBe("alice");
  });
});

describe("logging in", () => {
  it("logs in and remembers the relay", async () => {
    vi.mocked(api.logIn).mockResolvedValue(account({ server_url: RELAY }));
    form.serverUrl = ` ${RELAY} `;
    await form.submit();
    expect(api.logIn).toHaveBeenCalledWith(RELAY, "alice", "correct horse");
    expect(onAuthenticated).toHaveBeenCalledWith(expect.objectContaining({ username: "alice" }));
    expect(rememberedServer()).toBe(RELAY);
    expect(form.submitting).toBe(false);
  });

  it("says why it failed and remembers nothing", async () => {
    vi.mocked(api.logIn).mockRejectedValue(new Error("Wrong username or password"));
    form.serverUrl = RELAY;
    await form.submit();
    expect(form.error).toBe("Wrong username or password");
    expect(onAuthenticated).not.toHaveBeenCalled();
    expect(rememberedServer()).toBe(DEFAULT_SERVER);
    expect(form.submitting).toBe(false);
  });
});

describe("a new password", () => {
  it("has to be typed twice the same", async () => {
    form.switchMode("signup");
    form.password = "correct horse";
    form.confirmPassword = "correct house";
    await form.submit();
    expect(form.error).toBe(t("auth.mismatch"));
    expect(api.signUp).not.toHaveBeenCalled();
  });

  it("creates the account and hands over its recovery key to show", async () => {
    vi.mocked(api.signUp).mockResolvedValue({ account: account(), recovery_key: "AAAA-BBBB" });
    form.switchMode("signup");
    form.password = form.confirmPassword = "correct horse";
    await form.submit();
    expect(api.signUp).toHaveBeenCalledWith(DEFAULT_SERVER, "alice", "correct horse");
    expect(onAuthenticated).toHaveBeenCalledWith(expect.anything(), {
      key: "AAAA-BBBB",
      reason: "signup",
    });
    expect(toast.warning).not.toHaveBeenCalled();
  });

  it("is set with the recovery key, which gives a new one", async () => {
    vi.mocked(api.recoverAccount).mockResolvedValue({
      account: account(),
      recovery_key: "CCCC-DDDD",
    });
    form.switchMode("recover");
    form.recoveryKey = "AAAA-BBBB";
    form.password = form.confirmPassword = "juniper walrus";
    await form.submit();
    expect(api.recoverAccount).toHaveBeenCalledWith(
      DEFAULT_SERVER,
      "alice",
      "AAAA-BBBB",
      "juniper walrus"
    );
    expect(onAuthenticated).toHaveBeenCalledWith(expect.anything(), {
      key: "CCCC-DDDD",
      reason: "recovered",
    });
  });

  it("warns when the relay is too old to give a recovery key", async () => {
    vi.mocked(api.signUp).mockResolvedValue({ account: account(), recovery_key: null });
    form.switchMode("signup");
    form.password = form.confirmPassword = "correct horse";
    await form.submit();
    expect(toast.warning).toHaveBeenCalled();
    expect(onAuthenticated).toHaveBeenCalledWith(expect.anything(), undefined);
  });
});

describe("logging in with another device", () => {
  it("logs in with the code scanned", async () => {
    vi.mocked(scanQrCode).mockResolvedValue("ezcount://login?code=1");
    vi.mocked(api.logInWithLink).mockResolvedValue(account({ server_url: RELAY }));
    const done = form.scanToLogIn();
    expect(dialogs.scanning).toBe(true);
    await done;
    expect(dialogs.scanning).toBe(false);
    expect(api.logInWithLink).toHaveBeenCalledWith("ezcount://login?code=1");
    expect(onAuthenticated).toHaveBeenCalled();
    expect(rememberedServer()).toBe(RELAY);
  });

  it("does nothing when the scan is cancelled, and says why it couldn't scan", async () => {
    vi.mocked(scanQrCode).mockRejectedValueOnce(new ScanCancelled());
    await form.scanToLogIn();
    expect(form.error).toBeNull();
    vi.mocked(scanQrCode).mockRejectedValueOnce(new Error("The camera is needed"));
    await form.scanToLogIn();
    expect(form.error).toBe("The camera is needed");
    expect(api.logInWithLink).not.toHaveBeenCalled();
    expect(dialogs.scanning).toBe(false);
  });

  it("says when the code no longer works", async () => {
    vi.mocked(scanQrCode).mockResolvedValue("ezcount://login?code=1");
    vi.mocked(api.logInWithLink).mockRejectedValue(new Error("This link was used already"));
    await form.scanToLogIn();
    expect(form.error).toBe("This link was used already");
    expect(form.submitting).toBe(false);
  });

  it("takes the account a phone sent to the code this device shows", () => {
    form.received({ account: null, group: null });
    expect(onAuthenticated).not.toHaveBeenCalled();
    form.received({ account: account({ server_url: RELAY }), group: null });
    expect(onAuthenticated).toHaveBeenCalledWith(expect.objectContaining({ server_url: RELAY }));
    expect(rememberedServer()).toBe(RELAY);
  });
});
