use chrono::Local;
use clap::{arg, command, value_parser, Command};
use pinv::b64;
use pinv::db::{Catagory, CatagoryField, DataType, Db, Entry, EntryField};
use pinv::tui::Tui;
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
        .subcommand(
            // Find subcommand
            Command::new("find")
                .about("Find an entry given a key")
                .args(&[arg!([KEY] "The key of the entry to look up").required(true)]),
        )
        .subcommand(
            // Delete subcommand
            Command::new("delete")
                .about("Delete an entry given a key")
                .args(&[arg!([KEY] "The key of the entry to delete").required(true)]),
        )
        .subcommand(
            // Give subcommand
            Command::new("give")
                .about("Add to the quantity of an entry")
                .args(&[
                    arg!(-k --key <KEY> "The key of the entry to give to"),
                    arg!([QUANTITY] "The quantity to add to the entry")
                        .required(true)
                        .value_parser(value_parser!(u64)),
                ]),
        )
        .subcommand(
            // Take subcommand
            Command::new("take")
                .about("Take from the quantity of an entry")
                .args(&[
                    arg!(-k --key <KEY> "The key of the entry to take from"),
                    arg!([QUANTITY] "The quantity to take from the entry")
                        .required(true)
                        .value_parser(value_parser!(u64)),
                ]),
        )
        .subcommand(
            // Add subcommand
            Command::new("modify")
                .about("Modify an entry given a key")
                .args(&[
                    arg!(-k --key <KEY> "The key of the entry to modify.").required(true),
                    arg!([FIELD] ... "A field to modify in the entry.").required(true),
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
        // Find subcommand
        Some(("find", matches)) => {
            let key: String = matches.get_one::<String>("KEY").unwrap().clone();

            // Convert the key from b64 to u64
            let key = b64::to_u64(&key).unwrap();

            let entry = db.grab_entry(key).unwrap();

            println!("{}", entry);
        }
        // Delete subcommand
        Some(("delete", matches)) => {
            let key: String = matches.get_one::<String>("KEY").unwrap().clone();

            // Convert the key from b64 to u64
            let key = b64::to_u64(&key).unwrap();

            let entry = db.grab_entry(key).unwrap();

            println!(
                "{}\n\n\tONCE AN ENTRY IS DELETED, IT CANNOT BE UNDONE",
                entry
            );

            match confirm() {
                true => {}
                false => {
                    return;
                }
            }

            db.delete_entry(key).unwrap();
        }
        // Give subcommand
        Some(("give", matches)) => {
            let key: String = matches.get_one::<String>("key").unwrap().clone();
            let quantity: u64 = *matches.get_one::<u64>("QUANTITY").unwrap();

            // Convert the key from b64 to u64
            let key = b64::to_u64(&key).unwrap();

            let entry = db.grab_entry(key).unwrap();

            let new_quantity = entry.quantity + quantity;
            println!("{}", entry);

            println!("New quantity: {}", new_quantity);

            match confirm() {
                true => {}
                false => {
                    return;
                }
            }

            // Convert the new quantity to an entry field and submit...
            let field = EntryField::new("QUANTITY", &new_quantity.to_string());

            db.mod_entry(key, vec![field]).unwrap();
        }
        // Take subcommand
        Some(("take", matches)) => {
            let key: String = matches.get_one::<String>("key").unwrap().clone();
            let quantity: u64 = *matches.get_one::<u64>("QUANTITY").unwrap();

            // Convert the key from b64 to u64
            let key = b64::to_u64(&key).unwrap();

            let entry = db.grab_entry(key).unwrap();

            let new_quantity = match entry.quantity > quantity {
                true => entry.quantity - quantity,
                false => 0,
            };

            println!("{}", entry);

            println!("New quantity: {}", new_quantity);

            match confirm() {
                true => {}
                false => {
                    return;
                }
            }

            // Convert the new quantity to an entry field and submit...
            let field = EntryField::new("QUANTITY", &new_quantity.to_string());

            db.mod_entry(key, vec![field]).unwrap();
        }
        // Modify subcommand
        Some(("modify", matches)) => {
            let key: String = matches.get_one::<String>("key").unwrap().clone();
            let fields: Vec<String> = matches
                .get_many::<String>("FIELD")
                .unwrap()
                .cloned()
                .collect();

            // Convert the key from base64 to u64
            let key = b64::to_u64(&key).unwrap();

            let mut entry_fields: Vec<EntryField> = Vec::new();
            // Parse all the fields
            for field in fields {
                let (field_id, field_value) = split_field(&field).unwrap();

                let entry_field = EntryField::new(&field_id, &field_value);

                entry_fields.push(entry_field);
            }

            // Grab the entry (to display)
            let entry = db.grab_entry(key).unwrap();

            println!("Old Entry:\n\n{}\n\nModified Fields:\n\n", entry);
            // Get the fields that have been modified
            for field in &entry_fields {
                // Make sure the field isn't one of the hard-coded fields
                match field.id.as_str() {
                    "KEY" => println!("\tKEY: {} -> {}", b64::from_u64(entry.key), field.value),
                    "LOCATION" => println!("\t LOCATION: {} -> {}", entry.location, field.value),
                    "QUANTITY" => println!("\t QUANTITY: {} -> {}", entry.quantity, field.value),
                    "CREATED" | "MODIFIED" => {
                        panic!("Cannot alter the time of creation or modification!")
                    }
                    _ => {
                        // Get the old field
                        let old_field = entry
                            .fields
                            .iter()
                            .find(|old_field| old_field.id == field.id)
                            .unwrap();

                        println!("\t{}: {} -> {}", old_field.id, old_field.value, field.value);
                    }
                };
            }

            match confirm() {
                true => {}
                false => {
                    return;
                }
            }

            db.mod_entry(key, entry_fields).unwrap();
        }
        _ => {
            panic!("Exhausted list of subcommands and subcommand_required prevents `None`");
        }
    }
}
