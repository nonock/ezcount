import { render } from "@testing-library/svelte";
import { describe, expect, it } from "vitest";
import { group, participant } from "@/test/fixtures";
import MemberAvatar from "./MemberAvatar.svelte";

describe("MemberAvatar", () => {
  it("shows a member's initial on their color", () => {
    const { container } = render(MemberAvatar, { group: group(), participantId: "bob" });
    expect(container.textContent?.trim()).toBe("B");
    expect(container.querySelector(".tone-1")).not.toBeNull();
  });

  it("shows their picture when they have one", () => {
    const avatar = "data:image/webp;base64,AAAA";
    const trip = group({ participants: [participant("alice", { avatar })] });
    const { container } = render(MemberAvatar, { group: trip, participantId: "alice" });
    expect(container.querySelector("img")?.getAttribute("src")).toBe(avatar);
    expect(container.textContent?.trim()).toBe("");
  });

  it("takes the name of someone the group no longer lists", () => {
    const { container } = render(MemberAvatar, {
      group: group(),
      participantId: "gone",
      name: "zoe",
    });
    expect(container.textContent?.trim()).toBe("Z");
    expect(container.querySelector("[class*='tone-']")).toBeNull();
  });

  it("is hidden from screen readers, the name being next to it", () => {
    const { container } = render(MemberAvatar, { group: group(), participantId: "alice" });
    expect(container.querySelector("[aria-hidden='true']")).not.toBeNull();
  });
});
