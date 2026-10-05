import { render, screen } from "@testing-library/svelte";
import { describe, expect, it } from "vitest";
import { t } from "@/lib/i18n/index.svelte";
import UpdateNotice from "./UpdateNotice.svelte";

describe("UpdateNotice", () => {
  it("says the app is too old, and what waits for its update", () => {
    render(UpdateNotice, { text: "Your groups wait." });
    const notice = screen.getByTestId("update-notice");
    expect(notice.textContent).toContain(t("update.title"));
    expect(notice.textContent).toContain("Your groups wait.");
  });

  it("leaves updating to where the app came from: only the web version reloads", () => {
    render(UpdateNotice, { text: "Your groups wait." });
    expect(screen.queryByRole("button")).toBeNull();
  });
});
