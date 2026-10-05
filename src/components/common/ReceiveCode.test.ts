import { fireEvent, render, screen } from "@testing-library/svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { t } from "@/lib/i18n/index.svelte";
import { api } from "@/services/api";
import { account } from "@/test/fixtures";
import ReceiveCode from "./ReceiveCode.svelte";

vi.mock("@/services/api", () => ({ api: { receiveLink: vi.fn(), receive: vi.fn() } }));

const RELAY = "https://relay.example.com";
const LINK = "ezcount://receive?server=relay&code=1&for=login";

beforeEach(() => {
  vi.clearAllMocks();
  vi.useFakeTimers();
  vi.mocked(api.receiveLink).mockResolvedValue(LINK);
  vi.mocked(api.receive).mockResolvedValue(null);
});

afterEach(() => vi.useRealTimers());

describe("ReceiveCode", () => {
  it("shows a code for the phone to scan, made for this relay and purpose", async () => {
    render(ReceiveCode, { serverUrl: RELAY, purpose: "login", onReceived: vi.fn() });
    await vi.advanceTimersByTimeAsync(0);
    expect(api.receiveLink).toHaveBeenCalledWith(RELAY, "login");
    expect(screen.getByAltText(t("receive.qrAlt")).getAttribute("src")).toMatch(
      /^data:image\/svg\+xml/
    );
    expect(screen.getByText(t("receive.waiting"))).toBeTruthy();
  });

  it("asks the relay every two seconds, and hands over what the phone sent", async () => {
    const onReceived = vi.fn();
    const sent = { account: account(), group: null };
    vi.mocked(api.receive).mockResolvedValueOnce(null).mockResolvedValueOnce(sent);
    render(ReceiveCode, { serverUrl: RELAY, purpose: "login", onReceived });
    await vi.advanceTimersByTimeAsync(2000);
    expect(api.receive).toHaveBeenCalledTimes(1);
    expect(onReceived).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(2000);
    expect(api.receive).toHaveBeenLastCalledWith(LINK);
    expect(onReceived).toHaveBeenCalledWith(sent);
    // Once received, it stops asking.
    await vi.advanceTimersByTimeAsync(10_000);
    expect(api.receive).toHaveBeenCalledTimes(2);
  });

  it("stops asking when it leaves the screen", async () => {
    const { unmount } = render(ReceiveCode, {
      serverUrl: RELAY,
      purpose: "group",
      onReceived: vi.fn(),
    });
    await vi.advanceTimersByTimeAsync(2000);
    unmount();
    await vi.advanceTimersByTimeAsync(10_000);
    expect(api.receive).toHaveBeenCalledTimes(1);
  });

  it("expires after two minutes, and makes a new code when asked", async () => {
    render(ReceiveCode, { serverUrl: RELAY, purpose: "login", onReceived: vi.fn() });
    await vi.advanceTimersByTimeAsync(121_000);
    expect(screen.getByText(t("link.expired"))).toBeTruthy();
    expect(screen.queryByAltText(t("receive.qrAlt"))).toBeNull();
    await fireEvent.click(screen.getByRole("button", { name: t("link.again") }));
    await vi.advanceTimersByTimeAsync(0);
    expect(api.receiveLink).toHaveBeenCalledTimes(2);
    expect(screen.getByAltText(t("receive.qrAlt"))).toBeTruthy();
  });

  it("says why no code could be made", async () => {
    vi.mocked(api.receiveLink).mockRejectedValue(
      new Error("This sync server can't pass things between devices yet")
    );
    const { container } = render(ReceiveCode, {
      serverUrl: RELAY,
      purpose: "login",
      onReceived: vi.fn(),
    });
    await vi.advanceTimersByTimeAsync(0);
    expect(container.textContent).toContain(
      "This sync server can't pass things between devices yet"
    );
    expect(screen.getByRole("button", { name: t("link.again") })).toBeTruthy();
  });
});
