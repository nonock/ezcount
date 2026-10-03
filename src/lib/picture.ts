/** Side, in pixels, of the square a picture is shrunk to before it is saved. */
const SIDE = 256;

/**
 * A picture file as the small square `data:` URL the app stores: its middle, shrunk, as WebP
 * (or JPEG where the webview can't write WebP). A few kilobytes, since every member's device
 * keeps a copy. Throws for a file that isn't a picture.
 */
export async function pictureFromFile(file: File): Promise<string> {
  const bitmap = await createImageBitmap(file);
  try {
    const crop = Math.min(bitmap.width, bitmap.height);
    const canvas = document.createElement("canvas");
    canvas.width = canvas.height = Math.min(crop, SIDE);
    const context = canvas.getContext("2d");
    if (!context || crop === 0) throw new Error("No picture to draw");
    context.drawImage(
      bitmap,
      (bitmap.width - crop) / 2,
      (bitmap.height - crop) / 2,
      crop,
      crop,
      0,
      0,
      canvas.width,
      canvas.height
    );
    const webp = canvas.toDataURL("image/webp", 0.85);
    return webp.startsWith("data:image/webp") ? webp : canvas.toDataURL("image/jpeg", 0.85);
  } finally {
    bitmap.close();
  }
}
