import { fireEvent, render, screen } from "@testing-library/svelte";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { t } from "@/lib/i18n/index.svelte";
import { pictureFromFile } from "@/lib/picture";
import PictureField from "./PictureField.svelte";

vi.mock("@/lib/picture", () => ({ pictureFromFile: vi.fn() }));

const PICTURE = "data:image/webp;base64,AAAA";

/** Picks a file in the field, as the file chooser does. */
async function choose(name = "cat.png") {
  const input = screen.getByLabelText("Your picture") as HTMLInputElement;
  const file = new File(["…"], name, { type: "image/png" });
  Object.defineProperty(input, "files", { value: [file], configurable: true });
  await fireEvent.change(input);
  return file;
}

beforeEach(() => vi.clearAllMocks());

describe("PictureField", () => {
  it("offers to choose a picture while there is none", () => {
    render(PictureField, { value: null, label: "Your picture" });
    expect(screen.getByRole("button", { name: t("picture.choose") })).toBeTruthy();
    expect(screen.queryByTestId("picture-preview")).toBeNull();
    expect(screen.queryByRole("button", { name: t("picture.remove") })).toBeNull();
  });

  it("shows the picture chosen, shrunk as the app stores it", async () => {
    vi.mocked(pictureFromFile).mockResolvedValue(PICTURE);
    render(PictureField, { value: null, label: "Your picture" });
    const file = await choose();
    expect(pictureFromFile).toHaveBeenCalledWith(file);
    const preview = await screen.findByTestId("picture-preview");
    expect(preview.getAttribute("src")).toBe(PICTURE);
    expect(screen.getByRole("button", { name: t("picture.change") })).toBeTruthy();
  });

  it("removes the picture", async () => {
    render(PictureField, { value: PICTURE, label: "Your picture" });
    await fireEvent.click(screen.getByRole("button", { name: t("picture.remove") }));
    expect(screen.queryByTestId("picture-preview")).toBeNull();
  });

  it("says when a file isn't a picture, and keeps the one it had", async () => {
    vi.mocked(pictureFromFile).mockRejectedValue(new Error("not an image"));
    const { container } = render(PictureField, { value: PICTURE, label: "Your picture" });
    await choose("notes.txt");
    await vi.waitFor(() => expect(container.textContent).toContain(t("picture.unreadable")));
    expect(screen.getByTestId("picture-preview").getAttribute("src")).toBe(PICTURE);
  });
});
