# PID Format Analysis

This context defines the shared language for evidence-based analysis of SmartPlant / Smart P&ID packages and plant backups. It keeps format coverage claims separate from unproven semantics and write support.

## Language

### Evidence

**Evidence-Complete Read Parsing**:
A completeness level where every source byte is attributable to proven decoded data, an explicit evidence classification, or preserved unknown data. It does not require every byte to have business semantics and does not include semantic write-back.
_Avoid_: Full semantic decoding, read-write completeness, fully reverse-engineered format

**Decoded Data**:
Source bytes whose field boundaries and meanings are supported by bounded provenance and repeatable evidence.
_Avoid_: Recognized data, plausible data

**Typed Audit Data**:
Source bytes with a proven structural envelope but intentionally conservative or positional field meanings.
_Avoid_: Decoded data, unknown data

**Probe Evidence**:
Investigation-only observations or hypotheses that are useful for analysis but are not part of the stable decoded contract.
_Avoid_: Decoded data

**Identified-Only Data**:
A recognized stream or record family whose body does not yet have a proven structural decoder.
_Avoid_: Decoded data, unknown data

**Unknown Data**:
Source bytes without a proven classification or meaning that remain explicitly visible and are preserved unchanged.
_Avoid_: Ignored data, unsupported garbage

**Raw Preservation**:
The guarantee that source bytes outside proven semantic write surfaces remain available without inferred modification.
_Avoid_: Semantic write support

**Versioned Reference Corpus**:
The explicit, revision-controlled set of PID packages, symbol definitions, reference data, and controlled fixtures against which evidence completeness is measured. Files outside this corpus must remain safe to inspect but are not automatically covered by completeness claims.
_Avoid_: All vendor files, arbitrary-version compatibility

**Coverage Gap**:
A byte range, stream family, record family, or input variant that remains below the evidence level required by the current corpus contract.
_Avoid_: Parser failure, ignored data

**Controlled Fixture**:
A source file produced by one known authoring action so that its byte-level difference can support or reject a specific semantic claim.
_Avoid_: Representative sample, manually patched binary

### Plant Backups

**Plant Backup**:
The complete backup of one SmartPlant plant written by SmartPlant Engineering Manager: a `<Plant>_p` folder, or the zip archive of that folder exactly as written.
_Avoid_: Backup file, backup folder, backup package

**Database Dump**:
The `Export.dmp` in a Plant Backup that holds the plant's database schemas, either as an MTF Backup (SQL Server) or as an Oracle Export (`exp`).
_Avoid_: DMP file, database backup

**Option Archive**:
A file in a Plant Backup named `PlantData~<schema>~<id>` or `RefData~<schema>~<id>` that carries the folder or file one plant option path pointed at when the backup was taken.
_Avoid_: PlantData zip, RefData zip

**Backup Store**:
The SQLite database built from exactly one Plant Backup.
_Avoid_: Mirror, cache, backup database

**Schema Role**:
The part one schema in a Database Dump plays for its plant: Plant, Plant Dictionary, P&ID, or P&ID Dictionary. It names what the schema holds, unlike the schema's own name, which differs from plant to plant.
_Avoid_: Schema name, owner

**Ghost Row**:
A row deleted from a dumped table whose bytes are still in the Database Dump because the database had not yet reclaimed them. It is kept as evidence and never counted as table content.
_Avoid_: Deleted row, dead row, tombstone
