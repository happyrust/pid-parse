use futures_lite::stream::StreamExt;
use oxidized_mdf::MdfDatabase;
use prettytable::{Cell, Row, Table};
use std::path::PathBuf;
use structopt::StructOpt;

async fn print_rows(db: &mut MdfDatabase, table: &str, row_limit: &Option<usize>) {
    let mut rows = match db.rows(&table) {
        Some(rows) => rows,
        None => {
            eprintln!("No table {}", table);
            return;
        }
    };

    let mut pretty_table = Table::new();

    let mut i = 0usize;
    while let Some(row) = rows.next().await {
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

#[async_std::main]
async fn main() {
    femme::with_level(log::LevelFilter::Trace);
    let opt = Opts::from_args();

    let mut db = MdfDatabase::open(opt.path).await.unwrap();

    match opt.table {
        None => {
            for table in db.table_names() {
                print_rows(&mut db, &table, &opt.row_limit).await;
            }
        }
        Some(table) => {
            print_rows(&mut db, &table, &opt.row_limit).await;
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
