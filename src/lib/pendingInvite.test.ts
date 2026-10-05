import { afterEach, describe, expect, it, vi } from "vitest";
import { savedInvite, saveInvite } from "./pendingInvite";

const LINK = "ezcount://join?v=2&g=g1&k=secret";
const DAY = 24 * 3600 * 1000;

afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
  localStorage.clear();
});

describe("an invite waiting for the login", () => {
  it("is nothing until one is kept", () => {
    expect(savedInvite()).toBeNull();
  });

  it("is kept across restarts of the app", () => {
    saveInvite(LINK);
    expect(savedInvite()).toBe(LINK);
  });

  it("is forgotten once used", () => {
    saveInvite(LINK);
    saveInvite(null);
    expect(savedInvite()).toBeNull();
    expect(localStorage.length).toBe(0);
  });

  it("is too old after a week", () => {
    vi.useFakeTimers({ now: new Date(2026, 2, 1), toFake: ["Date"] });
    saveInvite(LINK);
    vi.setSystemTime(Date.now() + 7 * DAY - 1000);
    expect(savedInvite()).toBe(LINK);
    vi.setSystemTime(Date.now() + 2000);
    expect(savedInvite()).toBeNull();
  });

  it("ignores what isn't an invite it kept", () => {
    for (const stored of [
      "not json",
      "42",
      '{"at":0}',
      JSON.stringify({ link: 7, at: Date.now() }),
    ]) {
      localStorage.setItem("ezcount_pending_invite", stored);
      expect(savedInvite(), stored).toBeNull();
    }
  });

  it("only waits while the app stays open where nothing can be stored", () => {
    const refuse = () => {
      throw new Error("storage is off");
    };
    vi.stubGlobal("localStorage", { getItem: refuse, setItem: refuse, removeItem: refuse });
    expect(() => saveInvite(LINK)).not.toThrow();
    expect(savedInvite()).toBeNull();
  });
});
