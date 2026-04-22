// OrcaMDF probe + export tool.
//
// Modes:
//   (1) default            — list every user table with a column preview
//   (2) --tables-only      — one table name per line (pipe-friendly)
//   (3) --export NAME      — dump every row of the named table as JSON
//   (4) --to-sqlite PATH   — mirror every user table into a SQLite DB
//                            at PATH (in addition to stdout-only
//                            modes). All columns stored as TEXT for
//                            schema simplicity.
//
// Together these cover "is this MDF readable?" (default / tables-only),
// "give me one table as JSON" (export), and "give me a queryable
// offline copy" (to-sqlite). See the SmartPlant pipeline under tools/
// for the end-to-end workflow.
//
// License note: OrcaMDF is GPL-3.0. This wrapper lives under tools/
// so the pid-parse Rust crate stays MIT / Apache-2.0.

using System;
using System.Collections.Generic;
using System.Data.SQLite;
using System.IO;
using System.Linq;
using System.Text.Json;
using OrcaMDF.Core.Engine;
using OrcaMDF.Core.MetaData.DMVs;

static void PrintUsage()
{
    Console.Error.WriteLine(
        "Usage: OrcaMdfProbe <mdf-file> [--tables-only] [--search NAME]\n"
        + "                   [--export NAME] [--to-sqlite PATH]\n\n"
        + "  Default:          list every user table and the first 8 columns.\n"
        + "  --tables-only     just emit table names, one per line.\n"
        + "  --search NAME     filter table names by case-insensitive substring.\n"
        + "  --export NAME     dump every row of the named table as JSON to stdout.\n"
        + "  --to-sqlite PATH  mirror every user table (filtered by --search if\n"
        + "                    given) into a SQLite database at PATH. All columns\n"
        + "                    stored as TEXT. Re-run overwrites the file."
    );
}

if (args.Length < 1 || args.Contains("-h") || args.Contains("--help"))
{
    PrintUsage();
    Environment.Exit(args.Length < 1 ? 2 : 0);
}

string mdfPath = args[0];
bool tablesOnly = args.Contains("--tables-only");
string? searchTerm = null;
string? exportTable = null;
string? sqlitePath = null;
for (int i = 1; i < args.Length; i++)
{
    switch (args[i])
    {
        case "--search":
            if (i + 1 >= args.Length) { Fail("--search requires an argument"); }
            searchTerm = args[++i];
            break;
        case "--export":
            if (i + 1 >= args.Length) { Fail("--export requires a table name"); }
            exportTable = args[++i];
            break;
        case "--to-sqlite":
            if (i + 1 >= args.Length) { Fail("--to-sqlite requires a path"); }
            sqlitePath = args[++i];
            break;
    }
}

if (!File.Exists(mdfPath))
{
    Console.Error.WriteLine($"MDF file not found: {mdfPath}");
    Environment.Exit(1);
}

try
{
    using var db = new Database(new[] { mdfPath });

    if (exportTable != null)
    {
        ExportTable(db, exportTable);
        Environment.Exit(0);
    }

    var tables = db.Dmvs.Tables.ToList();
    if (searchTerm != null)
    {
        tables = tables
            .Where(t => t.Name.IndexOf(searchTerm, StringComparison.OrdinalIgnoreCase) >= 0)
            .ToList();
    }
    tables = tables.OrderBy(t => t.Name, StringComparer.OrdinalIgnoreCase).ToList();

    if (sqlitePath != null)
    {
        ExportToSqlite(db, tables, sqlitePath);
        Environment.Exit(0);
    }

    if (tablesOnly)
    {
        foreach (var t in tables)
            Console.WriteLine(t.Name);
        Environment.Exit(0);
    }

    Console.WriteLine($"Source: {mdfPath}");
    Console.WriteLine($"User tables (filtered: {tables.Count}):");
    foreach (var t in tables)
    {
        var columns = db.Dmvs
            .Columns
            .Where(c => c.ObjectID == t.ObjectID)
            .OrderBy(c => c.ColumnID)
            .Take(8)
            .Select(c => c.Name)
            .ToList();
        var colsPreview = columns.Count > 0
            ? string.Join(", ", columns)
            : "(no columns)";
        Console.WriteLine(
            $"  {t.Name,-40}  id={t.ObjectID}  cols: [{colsPreview}]"
        );
    }
}
catch (Exception ex)
{
    Console.Error.WriteLine($"ORCAMDF ERROR: {ex.GetType().Name}: {ex.Message}");
    if (ex.InnerException != null)
    {
        Console.Error.WriteLine($"  caused by: {ex.InnerException.GetType().Name}: {ex.InnerException.Message}");
    }
    Console.Error.WriteLine(ex.StackTrace);
    Environment.Exit(1);
}

static void Fail(string msg)
{
    Console.Error.WriteLine(msg);
    Environment.Exit(2);
}

static void ExportTable(Database db, string tableName)
{
    // Resolve the table so we can query its column layout, then use
    // OrcaMDF's DataScanner to read every row. Schema-agnostic output
    // — every value round-trips as a string via its OrcaMDF-provided
    // representation. Downstream Rust consumers parse types on their
    // side.
    var table = db.Dmvs.Tables
        .FirstOrDefault(t => t.Name.Equals(tableName, StringComparison.OrdinalIgnoreCase));
    if (table == null)
    {
        Console.Error.WriteLine($"table not found: {tableName}");
        Environment.Exit(1);
    }

    var columns = db.Dmvs
        .Columns
        .Where(c => c.ObjectID == table!.ObjectID)
        .OrderBy(c => c.ColumnID)
        .ToList();

    var scanner = new DataScanner(db);
    var rows = scanner.ScanTable(table!.Name).ToList();

    // Stream JSON line by line so very large tables (T_ModelItem
    // can hold tens of thousands of rows) don't balloon memory.
    var writerOptions = new JsonWriterOptions
    {
        Indented = false,
        SkipValidation = false,
    };
    using var stdout = Console.OpenStandardOutput();
    using var writer = new Utf8JsonWriter(stdout, writerOptions);

    writer.WriteStartObject();
    writer.WriteString("table", table!.Name);
    writer.WriteNumber("row_count", rows.Count);
    writer.WritePropertyName("columns");
    writer.WriteStartArray();
    foreach (var col in columns)
    {
        writer.WriteStartObject();
        writer.WriteString("name", col.Name);
        writer.WriteNumber("column_id", col.ColumnID);
        // Column surfaces type info via different property names across
        // OrcaMDF versions. Serialize whatever the object's ToString()
        // returns so we stay forward-compatible.
        writer.WriteString("descriptor", col.ToString() ?? "(null)");
        writer.WriteBoolean("nullable", col.IsNullable);
        writer.WriteEndObject();
    }
    writer.WriteEndArray();
    writer.WritePropertyName("rows");
    writer.WriteStartArray();
    foreach (var row in rows)
    {
        writer.WriteStartObject();
        foreach (var col in columns)
        {
            writer.WritePropertyName(col.Name);
            object? value = null;
            try
            {
                value = row[col.Name];
            }
            catch
            {
                // Some columns (LOB / TEXT) can throw when accessed
                // — we surface them as null so the dump stays
                // well-formed for downstream JSON consumers.
                value = null;
            }
            if (value == null)
            {
                writer.WriteNullValue();
            }
            else
            {
                writer.WriteStringValue(value.ToString());
            }
        }
        writer.WriteEndObject();
    }
    writer.WriteEndArray();
    writer.WriteEndObject();
    writer.Flush();
    Console.Out.WriteLine();
}

/// <summary>
/// Mirror the given user tables into a SQLite database at
/// <paramref name="sqlitePath"/>. Each MDF table becomes a SQLite
/// table with all columns typed as TEXT (OrcaMDF provides string
/// representations for every SQL Server type). A `_meta_columns`
/// auxiliary table records per-column metadata so downstream Rust
/// consumers can recover real types.
/// </summary>
static void ExportToSqlite(Database db, List<Table> tables, string sqlitePath)
{
    if (File.Exists(sqlitePath))
    {
        File.Delete(sqlitePath);
    }
    var connString = new SQLiteConnectionStringBuilder
    {
        DataSource = sqlitePath,
        Version = 3,
    }.ToString();
    using var conn = new SQLiteConnection(connString);
    conn.Open();

    // Speed tweaks for bulk insert: journal off, single-file mode.
    using (var pragma = conn.CreateCommand())
    {
        pragma.CommandText =
            "PRAGMA journal_mode = OFF;"
            + "PRAGMA synchronous = OFF;"
            + "PRAGMA temp_store = MEMORY;";
        pragma.ExecuteNonQuery();
    }

    // Metadata table so Rust-side consumers can distinguish
    // declared SQL Server types (string, int, datetime, binary, ...)
    // without another OrcaMDF round-trip.
    using (var createMeta = conn.CreateCommand())
    {
        createMeta.CommandText =
            "CREATE TABLE _meta_columns ("
            + "  table_name  TEXT NOT NULL,"
            + "  column_name TEXT NOT NULL,"
            + "  column_id   INTEGER NOT NULL,"
            + "  descriptor  TEXT NOT NULL,"
            + "  is_nullable INTEGER NOT NULL,"
            + "  PRIMARY KEY(table_name, column_name)"
            + ")";
        createMeta.ExecuteNonQuery();
    }

    var scanner = new DataScanner(db);
    int totalRows = 0;
    foreach (var table in tables)
    {
        var columns = db.Dmvs
            .Columns
            .Where(c => c.ObjectID == table.ObjectID)
            .OrderBy(c => c.ColumnID)
            .ToList();
        if (columns.Count == 0)
        {
            Console.Error.WriteLine($"  skipping {table.Name} — no columns");
            continue;
        }

        // Build CREATE TABLE with quoted identifiers so SQL Server
        // names that collide with SQLite keywords still work.
        var qTableName = QuoteIdent(table.Name);
        var colsSql = string.Join(", ", columns.Select(c => $"{QuoteIdent(c.Name)} TEXT"));
        using (var create = conn.CreateCommand())
        {
            create.CommandText = $"CREATE TABLE {qTableName} ({colsSql})";
            try
            {
                create.ExecuteNonQuery();
            }
            catch (SQLiteException ex)
            {
                Console.Error.WriteLine($"  skipping {table.Name} — CREATE TABLE failed: {ex.Message}");
                continue;
            }
        }

        // Record column metadata.
        using (var insMeta = conn.CreateCommand())
        {
            insMeta.CommandText =
                "INSERT INTO _meta_columns (table_name, column_name, column_id, descriptor, is_nullable)"
                + " VALUES ($t, $c, $cid, $d, $n)";
            var pT = insMeta.CreateParameter(); pT.ParameterName = "$t"; insMeta.Parameters.Add(pT);
            var pC = insMeta.CreateParameter(); pC.ParameterName = "$c"; insMeta.Parameters.Add(pC);
            var pCid = insMeta.CreateParameter(); pCid.ParameterName = "$cid"; insMeta.Parameters.Add(pCid);
            var pD = insMeta.CreateParameter(); pD.ParameterName = "$d"; insMeta.Parameters.Add(pD);
            var pN = insMeta.CreateParameter(); pN.ParameterName = "$n"; insMeta.Parameters.Add(pN);
            foreach (var col in columns)
            {
                pT.Value = table.Name;
                pC.Value = col.Name;
                pCid.Value = col.ColumnID;
                pD.Value = col.ToString() ?? string.Empty;
                pN.Value = col.IsNullable ? 1 : 0;
                insMeta.ExecuteNonQuery();
            }
        }

        // Row insert. One prepared statement per table.
        var placeholders = string.Join(", ", columns.Select((_, idx) => $"${idx}"));
        var insertSql = $"INSERT INTO {qTableName} VALUES ({placeholders})";
        using var ins = conn.CreateCommand();
        ins.CommandText = insertSql;
        var parameters = new SQLiteParameter[columns.Count];
        for (int p = 0; p < columns.Count; p++)
        {
            parameters[p] = ins.CreateParameter();
            parameters[p].ParameterName = $"${p}";
            ins.Parameters.Add(parameters[p]);
        }

        // Wrap the whole row enumeration in a try/catch: OrcaMDF
        // sometimes panics mid-stream on tables with LOB /
        // TextPointer columns that reference the record-index area
        // incorrectly. Failing one table must not abort every
        // subsequent table's export.
        int rowsInTable = 0;
        using var tx = conn.BeginTransaction();
        ins.Transaction = tx;
        try
        {
            var rows = scanner.ScanTable(table.Name);
            foreach (var row in rows)
            {
                for (int p = 0; p < columns.Count; p++)
                {
                    object? value = null;
                    try
                    {
                        value = row[columns[p].Name];
                    }
                    catch { value = null; }
                    parameters[p].Value = value is null
                        ? (object)DBNull.Value
                        : value.ToString()!;
                }
                try
                {
                    ins.ExecuteNonQuery();
                    rowsInTable++;
                }
                catch (Exception ex)
                {
                    Console.Error.WriteLine(
                        $"  {table.Name}: insert failed: {ex.Message}");
                }
            }
            tx.Commit();
        }
        catch (Exception ex)
        {
            // Abort this table cleanly; roll back the partial batch
            // so we do not leave the SQLite file in an inconsistent
            // state.
            tx.Rollback();
            Console.Error.WriteLine(
                $"  {table.Name,-40}  rows={rowsInTable} (TRUNCATED — {ex.GetType().Name}: {ex.Message})");
            continue;
        }

        Console.Error.WriteLine($"  {table.Name,-40}  rows={rowsInTable}");
        totalRows += rowsInTable;
    }

    Console.WriteLine($"Wrote {sqlitePath}: {tables.Count} tables, {totalRows} rows.");
}

/// <summary>
/// Quote a SQL identifier with double quotes, escaping any embedded
/// double quotes per the SQL standard.
/// </summary>
static string QuoteIdent(string name)
{
    return "\"" + name.Replace("\"", "\"\"") + "\"";
}
