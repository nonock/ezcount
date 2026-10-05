import { describe, expect, it, vi } from "vitest";
import { commands } from "../bindings";
import { group } from "../test/fixtures";
import { api } from "./api";

vi.mock("../bindings", () => ({
  commands: { getGroups: vi.fn(), getGroup: vi.fn(), deleteGroup: vi.fn() },
}));

describe("api", () => {
  it("returns what a command answered", async () => {
    vi.mocked(commands.getGroup).mockResolvedValue({ status: "ok", data: group() });
    await expect(api.getGroup("g1")).resolves.toMatchObject({ id: "g1" });
    expect(commands.getGroup).toHaveBeenCalledWith("g1");
  });

  it("throws a command's error", async () => {
    vi.mocked(commands.getGroup).mockResolvedValue({ status: "error", error: "Group not found" });
    await expect(api.getGroup("nope")).rejects.toThrow("Group not found");
  });

  it("passes on an answer that is nothing", async () => {
    vi.mocked(commands.deleteGroup).mockResolvedValue({ status: "ok", data: null });
    await expect(api.deleteGroup("g1")).resolves.toBeNull();
  });

  it("returns the groups of a command that can't fail", async () => {
    vi.mocked(commands.getGroups).mockResolvedValue([group()]);
    await expect(api.getGroups()).resolves.toHaveLength(1);
  });
});
