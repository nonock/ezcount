import { toast } from "svelte-sonner";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { api } from "@/services/api";
import { answerConfirm } from "@/test/confirm";
import { account, group } from "@/test/fixtures";
import { confirmation } from "../state/confirm.svelte";
import { dialogs } from "../state/dialogs.svelte";
import { openGroup } from "../state/groups.svelte";
import { session } from "../state/session.svelte";
import { addSelf, chooseIdentity, removeMember, skipIdentity } from "./members";

vi.mock("@/services/api", () => ({
  api: {
    getGroups: vi.fn().mockResolvedValue([]),
    getAccount: vi.fn(),
    removeParticipant: vi.fn(),
    setIdentity: vi.fn(),
    addSelf: vi.fn(),
  },
}));
vi.mock("@/services/native.svelte", () => ({ closeTopLayer: () => false }));
vi.mock("svelte-sonner", () => ({ toast: { error: vi.fn() } }));

beforeEach(() => {
  vi.clearAllMocks();
  confirmation.settle(false);
  session.account = account();
  openGroup.group = group();
  dialogs.who = true;
  dialogs.identitySkipped.clear();
  vi.spyOn(openGroup, "load").mockResolvedValue();
  vi.spyOn(openGroup, "change").mockImplementation(async (apply) => {
    await apply("g1");
  });
});

describe("removeMember", () => {
  it("asks before removing someone", async () => {
    const done = removeMember("bob");
    const asked = await answerConfirm(true);
    await done;
    expect(asked.title).toContain("Bob");
    expect(api.removeParticipant).toHaveBeenCalledWith("g1", "bob");
  });

  it("keeps them when the user says no, and says why removing failed", async () => {
    let done = removeMember("bob");
    await answerConfirm(false);
    await done;
    expect(api.removeParticipant).not.toHaveBeenCalled();

    vi.mocked(api.removeParticipant).mockRejectedValue(new Error("Participant not found"));
    done = removeMember("bob");
    await answerConfirm(true);
    await done;
    expect(toast.error).toHaveBeenCalledWith(expect.any(String), {
      description: "Participant not found",
    });
  });

  it("doesn't ask about someone the group doesn't list", async () => {
    await removeMember("nobody");
    expect(confirmation.open).toBe(false);
  });
});

describe("chooseIdentity", () => {
  it("records who the user is and reloads the group, which now shows their profile", async () => {
    vi.mocked(api.setIdentity).mockResolvedValue(account({ identities: { g1: "bob" } }));
    await chooseIdentity("bob");
    expect(api.setIdentity).toHaveBeenCalledWith("g1", "bob");
    expect(openGroup.currentUserId).toBe("bob");
    expect(openGroup.load).toHaveBeenCalledWith("g1");
  });
});

describe("addSelf", () => {
  it("adds the user as a new member and refreshes the account", async () => {
    vi.mocked(api.getAccount).mockResolvedValue(account({ identities: { g1: "dan" } }));
    await addSelf("Dan");
    expect(api.addSelf).toHaveBeenCalledWith("g1", "Dan");
    expect(openGroup.currentUserId).toBe("dan");
  });
});

describe("skipIdentity", () => {
  it("closes the question and doesn't ask again in this group", () => {
    skipIdentity();
    expect(dialogs.who).toBe(false);
    expect(dialogs.identitySkipped.has("g1")).toBe(true);
  });

  it("has nothing to remember once the user said who they are", () => {
    session.account = account({ identities: { g1: "alice" } });
    skipIdentity();
    expect(dialogs.identitySkipped.has("g1")).toBe(false);
  });
});
