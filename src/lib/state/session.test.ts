import { beforeEach, describe, expect, it, vi } from "vitest";
import { api } from "@/services/api";
import { account } from "@/test/fixtures";
import { session } from "./session.svelte";

vi.mock("@/services/api", () => ({ api: { getAccount: vi.fn() } }));

beforeEach(() => {
  session.account = undefined;
});

describe("session", () => {
  it("is logged out until an account is known", () => {
    expect(session.loggedIn).toBe(false);
    session.account = null;
    expect(session.loggedIn).toBe(false);
    session.account = account();
    expect(session.loggedIn).toBe(true);
  });

  it("goes by the profile's name, or the username without one", () => {
    expect(session.name).toBe("");
    session.account = account({ display_name: null });
    expect(session.name).toBe("alice");
    session.account = account({ display_name: "Alice M." });
    expect(session.name).toBe("Alice M.");
  });

  it("knows the groups put away and who the user is in each", () => {
    session.account = account({ archived: ["g2"], identities: { g1: "alice" } });
    expect(session.isArchived("g2")).toBe(true);
    expect(session.isArchived("g1")).toBe(false);
    expect(session.identityIn("g1")).toBe("alice");
    expect(session.identityIn("g2")).toBeNull();
  });

  it("answers for a logged-out user", () => {
    session.account = null;
    expect(session.isArchived("g1")).toBe(false);
    expect(session.identityIn("g1")).toBeNull();
  });

  it("loads the account from the core", async () => {
    vi.mocked(api.getAccount).mockResolvedValue(account());
    await session.refresh();
    expect(session.account?.username).toBe("alice");
  });

  it("counts as logged out when the account can't be read", async () => {
    vi.spyOn(console, "error").mockImplementation(() => {});
    vi.mocked(api.getAccount).mockRejectedValue(new Error("Database is locked"));
    await session.refresh();
    expect(session.account).toBeNull();
  });
});
