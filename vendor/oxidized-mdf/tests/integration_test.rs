use chrono::{TimeZone, Utc};
use oxidized_mdf::{error::Error, MdfDatabase, Value};
use pretty_assertions::assert_eq;
use rstest::rstest;

#[rstest(
    file,
    db_name,
    case("spg_verein_TST.mdf", "spg_verein_TST"),
    case("AWLT2005.mdf", "AdventureWorksLT")
)]
fn database_name(file: &str, db_name: &str) -> Result<(), Error> {
    let db = MdfDatabase::open(format!("data/{}", file))?;
    assert_eq!(db.database_name(), db_name);
    Ok(())
}

#[rstest(
    file,
    table_names,
    case("spg_verein_TST.mdf", vec!["___Tool_Text", "tblDOSBAbteilungen", "tblDOSBExportBE", "tblDOSBExportJAFachverbandsweise", "tblDOSBExportJAGesamt", "tblDOSBExportJASportartenweise", "tblDOSBExportJB", "tblDOSBSportarten", "tblDOSBVerein", "tblDOSBVorstand", "tblIMPORTAbrechnung", "tblIMPORTAbrechnungArt", "tblIMPORTAbrechnungDetails", "tblIMPORTAbrechnungDetailsZeilen", "tblImportEmailParameter", "tblImportMTAbteilungBeitraege", "tblImportMTAenderungen", "tblImportMTEhrungen", "tblImportMTFunktionen", "tblImportMitglied", "tblImportSTAbteilungBeitraege", "tblImportSTAbteilungen", "tblImportSTBerufsgruppen", "tblImportSTBezirke", "tblImportSTEhrungen", "tblImportSTFunktionen", "tblImportSTVerein", "tblImportSelektionen", "tblImportUmsaetze", "tblImportZahlungseingaenge", "tblImportZahlungseingaengeFibu", "tbl_Abrechnung", "tbl_AbrechnungAbgerechnet", "tbl_AbrechnungArt", "tbl_AbrechnungDatum", "tbl_AbrechnungDetails", "tbl_AbrechnungDetailsZeilen", "tbl_AbrechnungDetailsZeilenARCHIV", "tbl_AbrechnungDetails_ARCHIV", "tbl_AbrechnungSicherung", "tbl_Abrechnung_ARCHIV", "tbl_Abrechnungen_SEPA_Dateien", "tbl_Abteilung", "tbl_Abteilung_Beitrag", "tbl_Aenderungsprotokoll", "tbl_Aenderungsprotokoll_Gruende", "tbl_Aenderungsprotokoll_MitgliedLoeschung", "tbl_Ansichten", "tbl_BE_Mitglied_AKTIV_JN", "tbl_Bankleitzahlen", "tbl_Beitragsart", "tbl_Beitragserhebung_Sortierung", "tbl_Beitragserhebungen", "tbl_Beitragserhebungen_ARCHIV", "tbl_Beitragserhebungen_Details", "tbl_Beitragserhebungen_Details_ARCHIV", "tbl_Beitragserhebungen_Nacharbeiten", "tbl_Beitragserhebungen_SEPA_Dateien", "tbl_Benutzerfelder", "tbl_Berufsgruppe", "tbl_Bezirk", "tbl_Buchungsschluessel", "tbl_DOSBCodierung", "tbl_DOSBFachverbaende", "tbl_DOSBFunktionen", "tbl_DOSBMitglied_Funktion", "tbl_DOSBSportarten", "tbl_DOSBStatistiktypen", "tbl_DOSB_VE", "tbl_DOSB_VO", "tbl_DateiImport", "tbl_DateiImportRohdaten", "tbl_Dateiimport_Fehler", "tbl_Ehrung", "tbl_EmailParameter", "tbl_Exportvorlagen", "tbl_ExportvorlagenFeldnamen", "tbl_Feldnamen", "tbl_Formulare", "tbl_Formularnamen", "tbl_Funktion", "tbl_H_Abteilung", "tbl_H_Abteilung_Beitrag", "tbl_H_Beitragsart", "tbl_H_Berufsgruppe", "tbl_H_Bezirk", "tbl_H_Ehrung", "tbl_H_Funktion", "tbl_H_Mitglied", "tbl_H_Mitglied_Abteilung_Beitrag", "tbl_H_Mitglied_Ehrung", "tbl_H_Mitglied_Funktion", "tbl_H_Umsaetze", "tbl_H_Verein", "tbl_H_Zahlungseingaenge", "tbl_ID", "tbl_ImportProtokoll", "tbl_Importzuordnungen", "tbl_Laender", "tbl_Mahnungen", "tbl_Mahnungen_ARCHIV", "tbl_Mahnungen_Details", "tbl_Mahnungen_Details_ARCHIV", "tbl_MassenaenderungFeldname", "tbl_Mitglied", "tbl_Mitglied_Abteilung_Beitrag", "tbl_Mitglied_Ehrung", "tbl_Mitglied_Funktion", "tbl_Mitglieder_GES_SEL", "tbl_PLZ", "tbl_Proxy", "tbl_SEPA_Ausfuehrungen", "tbl_Selektionen", "tbl_SelektionenDetails", "tbl_SelektionenDetailsFilterungSortierung", "tbl_Statistik_Altersgruppen", "tbl_Statistik_Mitgliedschaft", "tbl_Strassenverzeichnis", "tbl_Umsaetze", "tbl_Verein", "tbl_Voreinstellungen", "tbl_Zahlarten", "tbl_Zahlungseingaenge", "tbl_Zahlweise", "tbl__datenversion", "tbl_tempStatistikDaten", "tbl_tempStatistikDatenAusgetreten", "tbl_tempStatistikZahlungsdaten", "tbl_temp_EXPORT", "tbl_temp_LL_Daten", "tbl_temp_LL_Daten_Mitglieder", "tbl_temp_LL_Daten_Rechnung", "trace_xe_action_map", "trace_xe_event_map"]),
    case("AWLT2005.mdf", vec!["Address", "BuildVersion", "Customer", "CustomerAddress", "ErrorLog", "Product", "ProductCategory", "ProductDescription", "ProductModel", "ProductModelProductDescription", "SalesOrderDetail", "SalesOrderHeader"])
)]
fn tables(file: &str, table_names: Vec<&str>) -> Result<(), Error> {
    let db = MdfDatabase::open(format!("data/{}", file))?;

    let mut tables = db.table_names();
    tables.sort();

    assert_eq!(tables, table_names);

    Ok(())
}

#[rstest(
    file,
    table_name,
    column_names,
    case("AWLT2005.mdf", "Address", vec!["AddressID", "AddressLine1", "AddressLine2", "City", "CountryRegion", "ModifiedDate", "PostalCode", "StateProvince", "rowguid"]),
    case("spg_verein_TST.mdf", "tbl_PLZ", vec!["Ort", "PLZ", "PLZID"]),
)]
fn columns(file: &str, table_name: &str, column_names: Vec<&str>) -> Result<(), Error> {
    let db = MdfDatabase::open(format!("data/{}", file))?;

    let mut columns = db.column_names(table_name).unwrap();
    columns.sort();

    assert_eq!(columns, column_names);

    Ok(())
}

#[rstest(
    file,
    table_name,
    column,
    value,
    case("AWLT2005.mdf", "Address", "AddressLine1", "8713 Yosemite Ct."),
    case("spg_verein_TST.mdf", "tbl_Mitglied", "Strasse", "Rebenring 56")
)]
fn first_row(file: &str, table_name: &str, column: &str, value: &str) -> Result<(), Error> {
    let mut db = MdfDatabase::open(format!("data/{}", file))?;

    let mut rows = db.rows(table_name).unwrap();
    let first_row = rows.next().unwrap();
    assert_eq!(
        first_row.value(column),
        Some(&Value::String(value.to_string()))
    );

    Ok(())
}

#[rstest(
    file,
    table_name,
    count,
    case("AWLT2005.mdf", "Address", 450),
    case("spg_verein_TST.mdf", "tbl_Mitglied", 13),
    // TODO: 3643 should be the correct number
    case("spg_verein_TST.mdf", "tbl_Bankleitzahlen", 3549)
)]
fn number_of_rows(file: &str, table_name: &str, count: usize) -> Result<(), Error> {
    let mut db = MdfDatabase::open(format!("data/{}", file))?;
    let rows = db.rows(table_name).unwrap();

    assert_eq!(rows.count(), count);

    Ok(())
}

#[rstest(
    file,
    table_name,
    skip,
    column,
    expected_value,
    case(
        "AWLT2005.mdf",
        "ProductCategory",
        0,
        "ParentProductCategoryID",
        Value::Null
    ),
    case(
        "AWLT2005.mdf",
        "ProductCategory",
        4,
        "ParentProductCategoryID",
        Value::Int(1)
    ),
    case(
        "spg_verein_TST.mdf",
        "tbl_Mitglied",
        1,
        "Titel",
        Value::String(String::from("Dr.Dr."))
    ),
    case("spg_verein_TST.mdf", "tbl_Mitglied", 3, "Titel", Value::Null),
    case("spg_verein_TST.mdf", "tbl_Mitglied", 0, "Kontosaldo", Value::Null),
    case("AWLT2005.mdf", "SalesOrderHeader", 0, "DueDate", Value::DateTime(Utc.with_ymd_and_hms(2004, 6, 13, 0, 0, 0).unwrap())),
    case("AWLT2005.mdf", "Product", 0, "SellEndDate", Value::Null),
    case("spg_verein_TST.mdf", "tbl_Mitglied", 0, "Austritt_Datum", Value::Null),
    case("spg_verein_TST.mdf", "tbl_Mitglied", 7, "Eintritt_Datum", Value::DateTime(Utc.with_ymd_and_hms(2006, 9, 10, 0, 0, 0).unwrap())),
)]
fn rows(
    file: &str,
    table_name: &str,
    skip: usize,
    column: &str,
    expected_value: Value,
) -> Result<(), Error> {
    let mut db = MdfDatabase::open(format!("data/{}", file))?;
    let mut rows = db.rows(table_name).unwrap().skip(skip);

    let row = rows.next().unwrap();

    assert_eq!(row.value(column), Some(&expected_value));

    Ok(())
}
