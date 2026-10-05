//! The format a document needs of the app: how a version keeps older ones from misreading
//! what it wrote, or from damaging it. See "Formats" in `doc`.

use super::*;

const FORMAT_MAP: &str = "format";

/// The format of a group this version of the app reads and writes.
pub const FORMAT: u32 = 1;

/// What changing a group in a newer format answers.
pub const NEWER_FORMAT: &str =
    "This group was changed by a newer version of ezcount. Update the app to open it.";

/// The format a document needs: the highest one a device marked it with (`require_format`),
/// and 1 for a document nobody marked, as all were at first.
pub fn format_needed(doc: &LoroDoc) -> u32 {
    let mut needed = 1;
    doc.get_map(FORMAT_MAP).for_each(|format, _| {
        if let Ok(format) = format.parse::<u32>() {
            needed = needed.max(format);
        }
    });
    needed
}

/// Marks a document as needing `format`: app versions that know an older one then leave it
/// alone and ask to be updated. Each format is a mark of its own, never taken back, so that
/// two devices marking at once both count.
pub fn require_format(doc: &LoroDoc, format: u32) -> Res<()> {
    if format_needed(doc) >= format {
        return Ok(());
    }
    doc.get_map(FORMAT_MAP)
        .insert(&format.to_string(), true)
        .map_err(doc_err)
}

/// Whether a group is in a format newer than this version of the app knows.
pub fn needs_newer_app(doc: &LoroDoc) -> bool {
    format_needed(doc) > FORMAT
}
