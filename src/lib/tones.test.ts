import { describe, expect, it } from "vitest";
import { group, participant } from "@/test/fixtures";
import { memberTone } from "./tones";

describe("memberTone", () => {
  const six = group({
    participants: ["a", "b", "c", "d", "e", "f"].map((id) => participant(id)),
  });

  it("colors a member by their position in the group", () => {
    expect(memberTone(six, "a")).toContain("tone-0");
    expect(memberTone(six, "c")).toContain("tone-2");
  });

  it("starts over after the last color", () => {
    expect(memberTone(six, "f")).toBe(memberTone(six, "a"));
  });

  it("gives someone the group doesn't list no color", () => {
    expect(memberTone(six, "nobody")).toBe("");
    expect(memberTone(six, undefined)).toBe("");
  });
});
