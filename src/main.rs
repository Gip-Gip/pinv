use chrono::Local;
use clap::{arg, command, value_parser, Command};
use pinv::db::{Catagory, CatagoryField, DataType, Db, Entry, EntryField};
use pinv::tui::Tui;
use pinv::{b64, csv};
use simple_error::bail;
use std::error::Error;
use std::fs;
use std::io::stdin;

fn confirm() -> bool {
    println!("Confirm?(y/n)");

    let mut answer = String::new();

    stdin().read_line(&mut answer).unwrap();

    if answer.trim() == "y" {
        return true;
    }
    eprintln!("'y' not selected, aborted!");
    false
}

fn split_field(field: &str) -> Result<(String, String), Box<dyn Error>> {
    // Split at the first "=", everything before will be the
    // field ID, everything after the field value
    let splitpoint = match field.find('=') {
        Some(splitpoint) => splitpoint,
        None => {
            bail!("Invalid field! No \"=\"!");
        }
    };

    let field_id = field[..splitpoint].to_uppercase();

    let field_value = field[splitpoint + 1..].to_owned();

    Ok((field_id, field_value))
}

/// Probably going to redo this in the near future, but it sorta works for now
fn main() {
    let mut db = Db::init();

    // To be re-written...
    let matches = command!()
        .propagate_version(true)
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(
            // TUI Subcommand
            Command::new("tui").about("Enter TUI mode"),
        )
        .subcommand(
            // Add subcommand
            Command::new("add")
                .about("Add an entry to a catagory")
                .args(&[
                    arg!(-c --catagory <CATAGORY> "The catagory to add the entry to.")
                        .required(true),
                    arg!(-k --key <KEY> "The key of the entry to add.").required(true),
                    arg!(-l --location <LOCATION> "The physical location of the entry.")
                        .required(true),
                    arg!(-q --quantity <QUANTITY> "The quantity of the entry.")
                        .required(true)
                        .value_parser(value_parser!(u64)),
                    arg!([FIELD] ... "A field to apply to the entry.").required(true),
                ]),
        )
        .subcommand(
            // Add catagory subcommand
            Command::new("add_catagory")
                .about("Add a new catagory")
                .args(&[
                    arg!(-c --catagory <CATAGORY> "The name of the catagory").required(true),
                    arg!([FIELD] ... "A field to apply to the catagory").required(true),
                ]),
        )
        .get_matches();

    match matches.subcommand() {
        // TUI Subcommand
        Some(("tui", _)) => {
            let mut tui = Tui::new(db).unwrap();

            tui.run();
        }
        // Add Subcommand
        Some(("add", matches)) => {
            let catagory: String = matches.get_one::<String>("catagory").unwrap().clone();
            let key: String = matches.get_one::<String>("key").unwrap().clone();
            let location: String = matches.get_one::<String>("location").unwrap().clone();
            let quantity: u64 = *matches.get_one::<u64>("quantity").unwrap();

            let fields: Vec<String> = matches
                .get_many::<String>("FIELD")
                .unwrap()
                .cloned()
                .collect();

            let mut entry_fields: Vec<EntryField> = Vec::new();
            // Parse all the fields
            for field in fields {
                let (field_id, field_value) = split_field(&field).unwrap();
                // Format the value
                let field_value = db
                    .format_string_to_field(&catagory, &field_id, &field_value)
                    .unwrap();

                let entry_field = EntryField::new(&field_id, &field_value);

                entry_fields.push(entry_field);
            }

            // Convert the key from base64 to u64
            let key = b64::to_u64(&key).unwrap();

            // Create the created/modified timestamp
            let created = Local::now().timestamp();
            let modified = created;

            let mut entry = Entry::new(&catagory, key, &location, quantity, created, modified);
            entry.add_fields(&entry_fields);

            println!("{}", entry);

            match confirm() {
                true => {}
                false => {
                    return;
                }
            }

            db.add_entry(entry).unwrap();
        }
        // Add catagory subcommand
        Some(("add_catagory", matches)) => {
            let catagory_id: String = matches.get_one::<String>("catagory").unwrap().clone();

            let fields: Vec<String> = matches
                .get_many::<String>("FIELD")
                .unwrap()
                .cloned()
                .collect();

            let mut catagory_fields: Vec<CatagoryField> = Vec::new();
            // Parse all the fields
            for field in fields {
                let (field_id, field_value) = split_field(&field).unwrap();

                if field_value.len() != 1 {
                    panic!(
                        "Catagory field is supposed to be one character, not {}!",
                        field_value
                    );
                }
                // Get the type
                let field_value = DataType::from_char(field_value.chars().next().unwrap()).unwrap();

                let catagory_field = CatagoryField::new(&field_id, field_value);

                catagory_fields.push(catagory_field);
            }

            let catagory = Catagory::with_fields(&catagory_id, catagory_fields);

            println!("{}", catagory);

            match confirm() {
                true => {}
                false => {
                    return;
                }
            }

            db.add_catagory(catagory).unwrap();
        }
        _ => {
            panic!("Exhausted list of subcommands and subcommand_required prevents `None`");
        }
    }
}
