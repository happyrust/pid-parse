use oxidized_mdf::MdfDatabase;
use prettytable::{Cell, Row, Table};
use std::path::PathBuf;
use structopt::StructOpt;

fn print_rows(db: &mut MdfDatabase, table: &str, row_limit: &Option<usize>) {
    let rows = match db.rows(table) {
        Some(rows) => rows,
        None => {
            eprintln!("No table {}", table);
            return;
        }
    };

    let mut pretty_table = Table::new();

    let mut i = 0usize;
    for row in rows {
        let values = row.values();

        if pretty_table.is_empty() {
            let cells = values.iter().map(|(k, _)| Cell::new(k)).collect::<Vec<_>>();
            pretty_table.add_row(Row::new(cells));
        }

        let cells = values
            .into_iter()
            .map(|(_, v)| Cell::new(&format!("{}", v)))
            .collect::<Vec<_>>();
        pretty_table.add_row(Row::new(cells));

        i += 1;

        if matches!(row_limit, Some(row_limit) if i >= *row_limit) {
            break;
        }
    }

    println!("--------------------");
    println!("Data of table: {}", table);
    println!("--------------------");
    pretty_table.printstd();
}

fn main() {
    let opt = Opts::from_args();

    let mut db = MdfDatabase::open(opt.path).unwrap();

    match opt.table {
        None => {
            for table in db.table_names() {
                print_rows(&mut db, &table, &opt.row_limit);
            }
        }
        Some(table) => {
            print_rows(&mut db, &table, &opt.row_limit);
        }
    }
}

#[derive(Debug, StructOpt)]
#[structopt(
    name = "print_all",
    about = "An example of how to read data from an MDF file."
)]
struct Opts {
    /// The path to the MDF file.
    #[structopt(parse(from_os_str))]
    path: PathBuf,

    /// Prints only the content of the given table
    #[structopt(long)]
    table: Option<String>,

    /// Max number of rows to print
    #[structopt(long)]
    row_limit: Option<usize>,
}
