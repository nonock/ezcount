import { describe, expect, it } from "vitest";
import { expense, group, participant } from "@/test/fixtures";
import { groupActivity } from "./activity";

describe("groupActivity", () => {
  it("ends with the group's creation by its first members", () => {
    const events = groupActivity(group());
    expect(events).toHaveLength(1);
    expect(events[0]).toMatchObject({ key: "created", action: "created", about: "member" });
    expect(events[0].text).toContain("Alice, Bob, and Carol");
  });

  it("says who added an expense and for how much", () => {
    const trip = group({
      expenses: [expense({ added_at: "2026-03-01T10:00:00Z", added_by: "bob" })],
    });
    expect(groupActivity(trip)[0]).toMatchObject({
      key: "e1-added",
      action: "added",
      text: "Bob added Taxi",
      detail: "€30",
    });
  });

  it("doesn't name anyone when the group doesn't know who did", () => {
    const trip = group({ expenses: [expense()] });
    // From before additions were dated: last touched when added.
    expect(groupActivity(trip)[0]).toMatchObject({
      at: "2026-03-01T10:00:00Z",
      text: "Taxi was added",
    });
  });

  it("lists edits with what changed, and the amount the expense was added with", () => {
    const edited = expense({
      amount_cents: 4000,
      added_at: "2026-03-01T10:00:00Z",
      added_by: "alice",
      history: [
        {
          edited_at: "2026-03-02T10:00:00Z",
          previous_title: "Taxi",
          previous_amount_cents: 3000,
          previous_paid_by: "alice",
          previous_splits: [],
          summary: "Amount changed from 30.00 to 40.00",
          edited_by: "bob",
        },
      ],
    });
    const [edit, added] = groupActivity(group({ expenses: [edited] }));
    expect(edit).toMatchObject({
      action: "edited",
      text: "Bob edited Taxi",
      detail: "Amount changed from 30.00 to 40.00",
    });
    expect(added.detail).toBe("€30");
  });

  it("tells a restored expense from an edited one", () => {
    const restored = expense({
      added_at: "2026-03-01T10:00:00Z",
      history: [
        {
          edited_at: "2026-03-03T10:00:00Z",
          previous_title: "Taxi",
          previous_amount_cents: 3000,
          previous_paid_by: "alice",
          previous_splits: [],
          summary: "Restored from the trash",
          edited_by: "carol",
        },
      ],
    });
    const [event] = groupActivity(group({ expenses: [restored] }));
    expect(event).toMatchObject({ action: "restored", text: "Carol restored Taxi" });
    expect(event.detail).toBeUndefined();
  });

  it("marks the occurrences the app added of a repeated expense", () => {
    const again = expense({
      id: "r1-2026-04-01",
      recurring: "r1",
      added_at: "2026-04-01T00:00:00Z",
      added_by: "alice",
    });
    expect(groupActivity(group({ expenses: [again] }))[0]).toMatchObject({
      action: "repeated",
      text: "Taxi was added again (repeated)",
    });
  });

  it("lists comments and deleted expenses", () => {
    const trip = group({
      expenses: [
        expense({
          added_at: "2026-03-01T10:00:00Z",
          comments: [{ id: "c1", text: "Receipt?", created_at: "2026-03-05T10:00:00Z", by: "bob" }],
        }),
      ],
      trash: [
        {
          expense: expense({ id: "e2", title: "Hotel", added_at: "2026-03-02T10:00:00Z" }),
          deleted_at: "2026-03-06T10:00:00Z",
          deleted_by: "alice",
        },
      ],
    });
    const events = groupActivity(trip);
    expect(events.map((e) => e.key)).toEqual([
      "e2-deleted",
      "comment-c1",
      "e2-added",
      "e1-added",
      "created",
    ]);
    expect(events[0].text).toBe("Alice deleted Hotel");
    expect(events[1]).toMatchObject({ text: "Bob commented on Taxi", detail: "Receipt?" });
  });

  it("says who joined, who was added and who was removed", () => {
    const trip = group({
      participants: [
        participant("alice"),
        participant("bob", { added_at: "2026-02-01T00:00:00Z", added_by: "bob" }),
        participant("carol", { added_at: "2026-02-02T00:00:00Z", added_by: "alice" }),
        participant("dan", {
          added_at: "2026-02-03T00:00:00Z",
          removed: true,
          removed_at: "2026-02-04T00:00:00Z",
          removed_by: "alice",
        }),
      ],
    });
    expect(groupActivity(trip).map((e) => e.text)).toEqual([
      "Alice removed Dan",
      "Dan was added",
      "Alice added Carol",
      "Bob joined the group",
      expect.stringContaining("Alice"),
    ]);
  });

  it("keeps the creation last whatever the dates of what was imported", () => {
    const trip = group({
      created_at: "2026-06-01T00:00:00Z",
      expenses: [expense({ added_at: "2020-01-01T00:00:00Z" })],
    });
    expect(groupActivity(trip).at(-1)?.action).toBe("created");
  });
});
