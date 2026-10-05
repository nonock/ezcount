import { fireEvent, render, screen } from "@testing-library/svelte";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { importGroup } from "@/lib/actions/groups";
import { t } from "@/lib/i18n/index.svelte";
import { session } from "@/lib/state/session.svelte";
import { account, equally, expense, group, participant } from "@/test/fixtures";
import GroupCard from "./GroupCard.svelte";
import GroupDashboard from "./GroupDashboard.svelte";

vi.mock("@/lib/actions/groups", () => ({ importGroup: vi.fn() }));
vi.mock("@/services/api", () => ({ api: {} }));

const trip = group({
  id: "trip",
  name: "Trip",
  expenses: [expense({ splits: equally("alice", "bob") })],
});
const flat = group({ id: "flat", name: "Flat" });

const handlers = () => ({
  onSelectGroup: vi.fn(),
  onOpenCreateGroup: vi.fn(),
  onOpenJoinGroup: vi.fn(),
});

beforeEach(() => {
  vi.clearAllMocks();
  session.account = account({ identities: { trip: "alice" } });
});

describe("GroupCard", () => {
  it("shows what the group spent and how many take part", () => {
    const onSelect = vi.fn();
    const { container } = render(GroupCard, { group: trip, net: 0, onSelect });
    expect(container.textContent).toContain("€30");
    expect(container.textContent).toContain(t("groups.summary", 3, 1));
    expect(screen.queryByTestId("group-net")).toBeNull();
  });

  it("opens the group", async () => {
    const onSelect = vi.fn();
    render(GroupCard, { group: trip, net: 0, onSelect });
    await fireEvent.click(screen.getByRole("button", { name: "Trip" }));
    expect(onSelect).toHaveBeenCalledWith("trip");
  });

  it("of a group a newer version of the app changed, shows the name and asks for an update", async () => {
    const onSelect = vi.fn();
    const newer = group({ id: "ski", name: "Ski", participants: [], needs_update: true });
    const { container } = render(GroupCard, { group: newer, net: 0, onSelect });
    expect(screen.getByTestId("group-needs-update").textContent).toContain(t("update.groupShort"));
    // Nothing is said of money that can't be read.
    expect(container.textContent).not.toContain(t("groups.totalSpent"));
    await fireEvent.click(screen.getByRole("button", { name: "Ski" }));
    expect(onSelect).toHaveBeenCalledWith("ski");
  });

  it("says what the user gets back, or owes", () => {
    const owed = render(GroupCard, { group: trip, net: 1500, onSelect: vi.fn() });
    expect(screen.getByTestId("group-net").textContent).toContain(t("groups.youGetBack"));
    expect(screen.getByTestId("group-net").querySelector(".text-positive")).not.toBeNull();
    owed.unmount();

    render(GroupCard, { group: trip, net: -1500, onSelect: vi.fn() });
    expect(screen.getByTestId("group-net").textContent).toContain(t("groups.youOwe"));
    expect(screen.getByTestId("group-net").textContent).toContain("€15");
  });

  it("doesn't count the members who left, and shows the group's picture and description", () => {
    const pictured = group({
      description: "Summer in Rome",
      image: "data:image/webp;base64,AAAA",
      participants: [participant("alice"), participant("bob", { removed: true })],
    });
    const { container } = render(GroupCard, { group: pictured, net: 0, onSelect: vi.fn() });
    expect(container.textContent).toContain(t("groups.summary", 1, 0));
    expect(container.textContent).toContain("Summer in Rome");
    expect(screen.getByTestId("group-picture").getAttribute("src")).toBe(pictured.image);
  });
});

describe("GroupDashboard", () => {
  it("offers to make, join or import a first group", async () => {
    const on = handlers();
    const { container } = render(GroupDashboard, { groups: [], ...on });
    expect(container.textContent).toContain(t("groups.empty"));
    await fireEvent.click(screen.getByRole("button", { name: t("groups.create") }));
    await fireEvent.click(screen.getByRole("button", { name: t("groups.joinWithCode") }));
    expect(on.onOpenCreateGroup).toHaveBeenCalled();
    expect(on.onOpenJoinGroup).toHaveBeenCalled();
  });

  it("lists the groups, with what the user is owed over all of them", () => {
    render(GroupDashboard, { groups: [trip, flat], ...handlers() });
    expect(screen.getByRole("button", { name: "Trip" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Flat" })).toBeTruthy();
    const overall = screen.getByRole("region", { name: t("groups.overall") });
    expect(overall.textContent).toContain("€15");
    expect(overall.querySelector(".text-positive")).not.toBeNull();
  });

  it("has no overall line while the user is owed nothing anywhere", () => {
    render(GroupDashboard, { groups: [flat], ...handlers() });
    expect(screen.queryByRole("region", { name: t("groups.overall") })).toBeNull();
  });

  it("lists the groups put away apart, and leaves them out of the totals", () => {
    session.account = account({ identities: { trip: "alice" }, archived: ["trip"] });
    const { container } = render(GroupDashboard, { groups: [trip, flat], ...handlers() });
    expect(container.textContent).toContain(t("groups.active", 1));
    expect(container.querySelector("details")?.textContent).toContain("Trip");
    expect(container.querySelector("details")?.textContent).not.toContain("Flat");
    expect(screen.queryByRole("region", { name: t("groups.overall") })).toBeNull();
  });

  it("imports the CSV file chosen", async () => {
    render(GroupDashboard, { groups: [flat], ...handlers() });
    const input = screen.getByLabelText(t("groups.csvFile")) as HTMLInputElement;
    const file = new File(["Date,Title"], "trip.csv", { type: "text/csv" });
    Object.defineProperty(input, "files", { value: [file], configurable: true });
    await fireEvent.change(input);
    expect(importGroup).toHaveBeenCalledWith(file);
  });
});
