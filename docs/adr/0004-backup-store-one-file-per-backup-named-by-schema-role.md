# A Backup Store is one SQLite file per Plant Backup, its tables named by Schema Role

Status: Accepted (grill-with-docs Q4, Q6, Q8, Q11, Q12, 2026-10-08/09)

Every Plant Backup becomes exactly one Backup Store and nothing else goes into that file, so each store answers to one immutable input and can be hashed, compared and rebuilt on its own; questions across backups use `ATTACH`. A dumped table is named after its Schema Role (`plant__`, `plantd__`, `pid__`, `pidd__`, from the connection type codes 2 / 8 / 4 / 9), not after the plant's own schema name, so one SQL statement runs against any plant's store; the original schema names are kept beside the tables. The store follows ADR-0001's evidence levels: only fields whose meaning is confirmed get a name, everything else is stored by key, position and raw value with its level, every dumped row records the page and slot it was read from, and Ghost Rows are kept byte-for-byte in a table of their own.

## Considered Options

- Many backups in one SQLite file keyed by a backup ID: rejected, no store could then be verified or rebuilt alone.
- Tables named by the plant's own schema (`"TEST02pid.T_Drawing"`): rejected, the name needs quoting and differs from plant to plant.
- One SQLite file per schema joined with `ATTACH`: rejected, it breaks one file per backup.
- Dropping Ghost Rows, or keeping them in the main tables behind a flag: rejected, the first throws away source bytes ADR-0001 keeps, the second puts deleted rows into ordinary queries.
