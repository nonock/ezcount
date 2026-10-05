import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { closeTopLayer } from "@/services/native.svelte";
import { navigation } from "./navigation.svelte";

vi.mock("@/services/native.svelte", () => ({ closeTopLayer: vi.fn(() => false) }));

/** What Back does to the page: the entry before, then the event. */
function goBack(state: unknown = null) {
  window.history.replaceState(state, "");
  window.dispatchEvent(new PopStateEvent("popstate", { state }));
}

let stop: () => void;

beforeEach(() => {
  window.history.replaceState(null, "");
  navigation.groupId = null;
  stop = navigation.listen();
});

afterEach(() => stop());

describe("navigation", () => {
  it("opens a group on its first tab, as a history entry", () => {
    navigation.tab = "stats";
    navigation.open("g1");
    expect(navigation.groupId).toBe("g1");
    expect(navigation.tab).toBe("expenses");
    expect(window.history.state).toEqual({ groupId: "g1" });
  });

  it("replaces the entry when another group opens over one", () => {
    const push = vi.spyOn(window.history, "pushState");
    navigation.open("g1");
    navigation.open("g2");
    expect(push).toHaveBeenCalledTimes(1);
    expect(window.history.state).toEqual({ groupId: "g2" });
  });

  it("leaves the group through the history", () => {
    const back = vi.spyOn(window.history, "back").mockImplementation(() => {});
    navigation.open("g1");
    navigation.close();
    expect(navigation.groupId).toBeNull();
    expect(back).toHaveBeenCalled();
  });

  it("doesn't go back when no group is in the history", () => {
    const back = vi.spyOn(window.history, "back").mockImplementation(() => {});
    navigation.close();
    expect(back).not.toHaveBeenCalled();
  });

  it("follows Back to the group list", () => {
    navigation.open("g1");
    goBack();
    expect(navigation.groupId).toBeNull();
  });

  it("follows Forward into the group", () => {
    goBack({ groupId: "g1" });
    expect(navigation.groupId).toBe("g1");
  });

  it("stays in the group when Back closed a dialog over it", () => {
    vi.mocked(closeTopLayer).mockReturnValueOnce(true);
    navigation.open("g1");
    goBack();
    expect(navigation.groupId).toBe("g1");
    expect(window.history.state).toEqual({ groupId: "g1" });
  });

  it("doesn't close a dialog when the app itself went back", () => {
    vi.spyOn(window.history, "back").mockImplementation(() => {});
    navigation.open("g1");
    navigation.close();
    goBack();
    expect(closeTopLayer).not.toHaveBeenCalled();
    expect(navigation.groupId).toBeNull();
  });
});
