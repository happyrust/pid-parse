//! Document-versioning `_Meta.xml` writer.

use std::fmt::Write;

use super::super::model::{PublishDrawing, PublishError};
use super::common::{
    escape_attr, fmt_err, CONTAINER_SCHEMA_VERSION, CONTAINER_SCOPE, CONTAINER_SDECIMAL,
    CONTAINER_SOFTWARE_VERSION, CONTAINER_TOOL_ID, CONTAINER_TOOL_SIGNATURE,
};

/// `_Meta.xml` switches the schema marker to advertise it as the
/// document-versioning sibling of the main data document. Reference
/// SPPID exports keep `Scope="Data"` for both files, so only the
/// schema label changes.
const CONTAINER_META_COMP_SCHEMA: &str = "DocVersioningComponent";
/// Default `<IDocumentVersion DocRevision="..."/>` attribute when a
/// drawing has not been versioned in the source backup. Reference
/// exports ship `"0"` for unrevised drawings.
const META_DEFAULT_DOC_REVISION: &str = "0";
/// Default `<IDocumentVersion DocVersion="..."/>` attribute. Same
/// rationale — reference exports use `"1"` for first-time emits.
const META_DEFAULT_DOC_VERSION: &str = "1";
/// Emit a `_Meta.xml` document for the given drawing. `SmartPlant`'s
/// reference Publish-Data export ships the document-versioning
/// envelope as a sibling file alongside `<DrawingName>_Data.xml`.
/// Compared with `write_data_xml` the structural shape is fixed and
/// minimal — three nodes (`DocumentVersion` / `DocumentRevision` /
/// File) and three `<Rel>` rows wiring them together.
///
/// The meta document carries no business attributes. Its sole
/// inputs are the drawing's `drawing_uid`, `drawing_name`, and
/// (optionally) `date_created`. Inner UIDs (the ones stamped onto
/// the version / revision / file / rel nodes) are derived
/// deterministically from `drawing_uid` via `derive_meta_uid` so
/// successive re-emits of the same drawing produce byte-identical
/// XML — a property tests rely on heavily.
///
/// `plant_name` is reused unchanged from the data document; it
/// shows up as the `<Container Plant="...">` attribute.
pub fn write_meta_xml(drawing: &PublishDrawing, plant_name: &str) -> Result<String, PublishError> {
    let mut buf = String::with_capacity(1024);
    writeln!(buf, r#"<?xml version ="1.0" encoding="UTF-8"?>"#).map_err(fmt_err)?;
    write_meta_container_open(&mut buf, drawing, plant_name)?;

    let version_uid = derive_meta_uid(&drawing.drawing_uid, "version");
    let revision_uid = derive_meta_uid(&drawing.drawing_uid, "revision");
    let file_uid = derive_meta_uid(&drawing.drawing_uid, "file");
    let rel_versioned_uid = derive_meta_uid(&drawing.drawing_uid, "rel/versioned-doc");
    let rel_revised_uid = derive_meta_uid(&drawing.drawing_uid, "rel/revised-document");
    let rel_file_uid = derive_meta_uid(&drawing.drawing_uid, "rel/file-composition");

    write_meta_document_version(&mut buf, drawing, &version_uid)?;
    write_meta_rel(
        &mut buf,
        &rel_versioned_uid,
        &drawing.drawing_uid,
        &version_uid,
        "VersionedDoc",
    )?;
    write_meta_document_revision(&mut buf, drawing, &revision_uid)?;
    write_meta_rel(
        &mut buf,
        &rel_revised_uid,
        &revision_uid,
        &drawing.drawing_uid,
        "RevisedDocument",
    )?;
    write_meta_file(&mut buf, drawing, &file_uid)?;
    write_meta_rel(
        &mut buf,
        &rel_file_uid,
        &file_uid,
        &version_uid,
        "FileComposition",
    )?;

    writeln!(buf, "  </Container>").map_err(fmt_err)?;
    Ok(buf)
}
/// `_Meta.xml` flavor of the container header. Identical wire shape
/// to [`write_container_open`] but stamps `CompSchema=
/// "DocVersioningComponent"` so `SmartPlant` routes the document to
/// the document-versioning loader instead of the data loader.
/// `LoginUser` / `LoginPWD` attributes are intentionally omitted to
/// match the reference exports byte-for-byte.
fn write_meta_container_open(
    buf: &mut String,
    drawing: &PublishDrawing,
    plant_name: &str,
) -> Result<(), PublishError> {
    writeln!(
        buf,
        concat!(
            r#"<Container CompSchema="{}" Scope="{}" SoftwareVersion="{}" "#,
            r#"IsValidated="False" SchemaVersion="{}" Plant="{}" Project="" "#,
            r#"DocUID="{}" DocName="{}" Version="" ToolID="{}" "#,
            r#"ToolSignature="{}" SDECIMAL="{}">"#
        ),
        CONTAINER_META_COMP_SCHEMA,
        CONTAINER_SCOPE,
        CONTAINER_SOFTWARE_VERSION,
        CONTAINER_SCHEMA_VERSION,
        escape_attr(plant_name),
        escape_attr(&drawing.drawing_uid),
        escape_attr(&drawing.drawing_name),
        CONTAINER_TOOL_ID,
        CONTAINER_TOOL_SIGNATURE,
        CONTAINER_SDECIMAL,
    )
    .map_err(fmt_err)
}

/// Emit `<DocumentVersion>` block with the deterministic
/// `version_uid`, the drawing's friendly name (`"<name> Version"`),
/// and a `DocVersionDate` parsed from `drawing.date_created`. When
/// no date is available the attribute renders empty rather than
/// fabricating a fake one — downstream tooling can treat the empty
/// string as "unknown".
fn write_meta_document_version(
    buf: &mut String,
    drawing: &PublishDrawing,
    version_uid: &str,
) -> Result<(), PublishError> {
    let version_date = drawing
        .date_created
        .as_deref()
        .map(format_meta_date)
        .unwrap_or_default();
    writeln!(buf, "   <DocumentVersion>").map_err(fmt_err)?;
    writeln!(
        buf,
        r#"      <IObject UID="{}" Name="{} Version"/>"#,
        escape_attr(version_uid),
        escape_attr(&drawing.drawing_name),
    )
    .map_err(fmt_err)?;
    writeln!(
        buf,
        r#"      <IDocumentVersion DocRevision="{}" DocVersionDate="{}" DocVersion="{}"/>"#,
        META_DEFAULT_DOC_REVISION,
        escape_attr(&version_date),
        META_DEFAULT_DOC_VERSION,
    )
    .map_err(fmt_err)?;
    writeln!(buf, "      <IFileComposition/>").map_err(fmt_err)?;
    writeln!(buf, "   </DocumentVersion>").map_err(fmt_err)
}

/// Emit `<DocumentRevision>` with the deterministic `revision_uid`.
/// `MajorRev_ForRevise` defaults to `"0"` and `MinorRev_ForRevise`
/// stays empty; both match every reference fixture and there is no
/// `SQLite` column carrying the values yet.
fn write_meta_document_revision(
    buf: &mut String,
    drawing: &PublishDrawing,
    revision_uid: &str,
) -> Result<(), PublishError> {
    writeln!(buf, "   <DocumentRevision>").map_err(fmt_err)?;
    writeln!(
        buf,
        r#"      <IObject UID="{}" Name="{} Revision"/>"#,
        escape_attr(revision_uid),
        escape_attr(&drawing.drawing_name),
    )
    .map_err(fmt_err)?;
    writeln!(
        buf,
        r#"      <IDocumentRevision MajorRev_ForRevise="0" MinorRev_ForRevise=""/>"#,
    )
    .map_err(fmt_err)?;
    writeln!(buf, "   </DocumentRevision>").map_err(fmt_err)
}

/// Emit `<File>` with the deterministic `file_uid`. The file name
/// is `<drawing_name>.pid` to mirror the on-disk artifact; the
/// `IFile FilePath=""` attribute stays empty because the original
/// SPPID export stamps the local filesystem path of the operator's
/// machine — a value we have no way of recovering and that, when
/// missing, downstream consumers tolerate.
fn write_meta_file(
    buf: &mut String,
    drawing: &PublishDrawing,
    file_uid: &str,
) -> Result<(), PublishError> {
    writeln!(buf, "   <File>").map_err(fmt_err)?;
    writeln!(
        buf,
        r#"      <IObject UID="{}" Name="{}.pid" Description=""/>"#,
        escape_attr(file_uid),
        escape_attr(&drawing.drawing_name),
    )
    .map_err(fmt_err)?;
    writeln!(buf, r#"      <IFile FilePath=""/>"#).map_err(fmt_err)?;
    writeln!(buf, "   </File>").map_err(fmt_err)
}

/// Emit a single `<Rel>` row. `def_uid` is the `SmartPlant`
/// relationship classifier (`"VersionedDoc"`, `"RevisedDocument"`,
/// `"FileComposition"`); the meta document only ever uses these
/// three so the helper does not need to be more general.
fn write_meta_rel(
    buf: &mut String,
    rel_uid: &str,
    uid1: &str,
    uid2: &str,
    def_uid: &str,
) -> Result<(), PublishError> {
    writeln!(buf, "   <Rel>").map_err(fmt_err)?;
    writeln!(buf, r#"      <IObject UID="{}"/>"#, escape_attr(rel_uid),).map_err(fmt_err)?;
    writeln!(
        buf,
        r#"      <IRel UID1="{}" UID2="{}" DefUID="{}"/>"#,
        escape_attr(uid1),
        escape_attr(uid2),
        escape_attr(def_uid),
    )
    .map_err(fmt_err)?;
    writeln!(buf, "   </Rel>").map_err(fmt_err)
}

/// Derive a deterministic 32-hex-character `SmartPlant` UID from
/// `(seed, role)` via UUID v5 (SHA-1 over the OID namespace). The
/// `seed` is typically the drawing's `T_Drawing.SP_ID`; the `role`
/// disambiguates the per-document children (`"version"`,
/// `"revision"`, `"file"`, `"rel/<def-uid>"`). The output is the
/// uppercase 32-char hex form `SmartPlant` uses for every `SP_ID`.
///
/// Determinism is the whole point: the writer can be invoked twice
/// and both runs produce byte-identical `_Meta.xml`, so test
/// fixtures can be golden-compared and CI can detect any drift.
pub(super) fn derive_meta_uid(seed: &str, role: &str) -> String {
    let payload = format!("{seed}/{role}");
    let uuid = uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, payload.as_bytes());
    uuid.simple().to_string().to_uppercase()
}

/// Normalize a `SmartPlant` `DateCreated` string into the
/// `YYYY/MM/DD` form reference exports use for `DocVersionDate`.
///
/// The MDF loader may surface the value as a SQL Server-style raw
/// render, for example `"2026/4/20 10:32:46"`. We zero-pad the
/// month and day and drop the time component, matching the reference
/// fixtures' `"2026/04/20"`. Any input that does not parse as
/// `YYYY/M/D[ ...]` is returned verbatim so callers retain enough
/// debug context to spot unsupported formats.
pub(super) fn format_meta_date(raw: &str) -> String {
    let date_part = raw.split_whitespace().next().unwrap_or(raw);
    let mut parts = date_part.split('/');
    let (Some(y), Some(m), Some(d), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return raw.to_string();
    };
    let (Ok(y_n), Ok(m_n), Ok(d_n)) = (y.parse::<u32>(), m.parse::<u32>(), d.parse::<u32>()) else {
        return raw.to_string();
    };
    format!("{y_n:04}/{m_n:02}/{d_n:02}")
}
