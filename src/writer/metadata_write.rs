//! Resolve [`MetadataUpdates`] into concrete stream replacements on a
//! [`PidPackage`].
//!
//! First-version scope is intentionally narrow: only the two well-known
//! `/TaggedTxtData/*` XML streams are touched. We deliberately do **not**
//! re-encode the bytes (no BOM detection, no UTF-16 conversion); callers
//! produce the exact bytes they want written, and this module performs a
//! straight `into_bytes()` swap.

use crate::error::PidError;
use crate::package::PidPackage;
use crate::writer::plan::MetadataUpdates;

const DRAWING_PATH: &str = "/TaggedTxtData/Drawing";
const GENERAL_PATH: &str = "/TaggedTxtData/General";

/// Apply the [`MetadataUpdates`] portion of a write plan to the in-memory
/// package. Returns `Ok(())` on success; only path/string handling can
/// fail today, so this signature is reserved for future cases (e.g.
/// SummaryInformation re-encoding).
pub fn apply_metadata_updates(
    package: &mut PidPackage,
    updates: &MetadataUpdates,
) -> Result<(), PidError> {
    if let Some(ref xml) = updates.drawing_xml {
        package.replace_stream(DRAWING_PATH, xml.clone().into_bytes());
    }
    if let Some(ref xml) = updates.general_xml {
        package.replace_stream(GENERAL_PATH, xml.clone().into_bytes());
    }
    // summary_updates: deferred to a future revision (see writer-layer-plan
    // §"Risks"). We accept the field today so callers can already encode
    // intent without breaking when support lands.
    let _ = &updates.summary_updates;
    Ok(())
}
