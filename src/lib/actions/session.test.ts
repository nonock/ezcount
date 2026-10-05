import { toast } from "svelte-sonner";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { api } from "@/services/api";
import { answerConfirm } from "@/test/confirm";
import { account, group } from "@/test/fixtures";
import { confirmation } from "../state/confirm.svelte";
import { dialogs } from "../state/dialogs.svelte";
import { groupList, openGroup } from "../state/groups.svelte";
import { navigation } from "../state/navigation.svelte";
import { session } from "../state/session.svelte";
import { deleteAccount, logOut } from "./session";

vi.mock("@/services/api", () => ({ api: { deleteAccount: vi.fn(), logOut: vi.fn() } }));
vi.mock("@/services/native.svelte", () => ({ closeTopLayer: () => false }));
vi.mock("svelte-sonner", () => ({ toast: { error: vi.fn(), success: vi.fn() } }));

beforeEach(() => {
  vi.clearAllMocks();
  confirmation.settle(false);
  window.history.replaceState(null, "");
  session.account = account();
  groupList.all = [group()];
  openGroup.group = group();
  navigation.groupId = "g1";
  dialogs.identitySkipped.add("g1");
  dialogs.createGroup = true;
  dialogs.expense = { open: true, editing: null };
});

const loggedOut = () => session.account === null;

describe("logOut", () => {
  it("asks, then forgets the account and its groups", async () => {
    vi.mocked(api.logOut).mockResolvedValue();
    const done = logOut();
    const asked = await answerConfirm(true);
    await done;
    expect(asked.title).toContain("alice");
    expect(api.logOut).toHaveBeenCalledWith(false);
    expect(loggedOut()).toBe(true);
    expect(groupList.all).toEqual([]);
    expect(openGroup.group).toBeNull();
    expect(navigation.groupId).toBeNull();
    expect(dialogs.identitySkipped.size).toBe(0);
    // Nothing is left open to come back with the next account.
    expect(dialogs.createGroup).toBe(false);
    expect(dialogs.expense.open).toBe(false);
  });

  it("stays logged in when the user says no", async () => {
    const done = logOut();
    await answerConfirm(false);
    await done;
    expect(api.logOut).not.toHaveBeenCalled();
    expect(loggedOut()).toBe(false);
  });

  it("says what would be lost, and logs out anyway only if asked to", async () => {
    vi.mocked(api.logOut).mockRejectedValueOnce(new Error("Some changes are not uploaded yet"));
    let done = logOut();
    await answerConfirm(true);
    const warning = await answerConfirm(false);
    await done;
    expect(warning.description).toBe("Some changes are not uploaded yet");
    expect(warning.destructive).toBe(true);
    expect(loggedOut()).toBe(false);

    vi.mocked(api.logOut).mockRejectedValueOnce(new Error("Some changes are not uploaded yet"));
    vi.mocked(api.logOut).mockResolvedValueOnce();
    done = logOut();
    await answerConfirm(true);
    await answerConfirm(true);
    await done;
    expect(api.logOut).toHaveBeenLastCalledWith(true);
    expect(loggedOut()).toBe(true);
  });

  it("stays logged in when even that fails", async () => {
    vi.mocked(api.logOut).mockRejectedValue(new Error("Database is locked"));
    const done = logOut();
    await answerConfirm(true);
    await answerConfirm(true);
    await done;
    expect(toast.error).toHaveBeenCalledWith(expect.any(String), {
      description: "Database is locked",
    });
    expect(loggedOut()).toBe(false);
  });

  it("does nothing when no one is logged in", async () => {
    session.account = null;
    await logOut();
    expect(confirmation.open).toBe(false);
  });
});

describe("deleteAccount", () => {
  it("deletes the account with its password, then forgets it and its groups", async () => {
    vi.mocked(api.deleteAccount).mockResolvedValue();
    await deleteAccount("correct horse");
    expect(api.deleteAccount).toHaveBeenCalledWith("correct horse");
    expect(loggedOut()).toBe(true);
    expect(groupList.all).toEqual([]);
    expect(openGroup.group).toBeNull();
    expect(navigation.groupId).toBeNull();
    expect(dialogs.createGroup).toBe(false);
    expect(toast.success).toHaveBeenCalled();
  });

  it("changes nothing when the core refuses, and says why to who asked", async () => {
    vi.mocked(api.deleteAccount).mockRejectedValue(new Error("Wrong password"));
    await expect(deleteAccount("nope")).rejects.toThrow("Wrong password");
    expect(loggedOut()).toBe(false);
    expect(groupList.all).toHaveLength(1);
    expect(navigation.groupId).toBe("g1");
    expect(toast.success).not.toHaveBeenCalled();
  });
});
