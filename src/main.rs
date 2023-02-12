use chrono::Local;
use clap::{arg, command, Command};
use pinv::db::{Catagory, CatagoryField, Db, Entry, EntryField};
use pinv::tui::Tui;
use pinv::{b64, csv};
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

/// Probably going to redo this in the near future, but it sorta works for now
fn main() {
    let mut db = Db::init();

    
    let mut tui = Tui::new(db).unwrap();

    tui.run();
    return;

    // To be re-written...
    let matches = command!()
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(Command::new("add").args(&[
            arg!(-k --key <KEY> "entry key").required(true),
            arg!(-c --catagory <CATAGORY> "catagory to insert the entry in").required(true),
            arg!(-l --location <LOCATION> "location the entry will be stored in").required(true),
            arg!(-q --quantity <QUANTITY> "quantity of the entry to be stored").required(true),
            arg!([FIELD] ... "foo"),
        ]))
        .subcommand(Command::new("import_csv").args(&[arg!([FILE] "csv file to import")]))
        .subcommand(
            Command::new("grab")
                .args(&[arg!([KEY] "key of the entry to be grabbed").required(true)]),
        )
        .subcommand(Command::new("add_catagory").args(&[
            arg!([FIELD] ... "field to add to the catagory").required(true),
            arg!(-c --catagory <CATAGORY> ... "catagory to add").required(true),
        ]))
        .subcommand(Command::new("list").args(&[
            arg!(-c --catagory <CATAGORY> "Catagory to list").required(true),
            arg!([CONSTRAINTS] ... "all the constraints").required(true),
        ]))
        .subcommand(Command::new("delete").args(&[arg!([KEY] "key of the entry to delete")]))
        .subcommand(Command::new("take").args(&[
            arg!([QUANTITY] "quantity to take from the entry").required(true),
            arg!(-k --key <KEY> "key of the entry to take from").required(true),
        ]))
        .subcommand(Command::new("give").args(&[
            arg!([QUANTITY] "quantity to give to the entry").required(true),
            arg!(-k --key <KEY> "key of the entry to give to").required(true),
        ]))
        .subcommand(Command::new("fill_template").args(&[
            arg!([IN] "template file").required(true),
            arg!(-o --out <FILE> "output file").required(true),
        ]))
        .subcommand(Command::new("tui"))
        .get_matches();

    match matches.subcommand() {
        Some(("tui", _)) => {

            return;
        }
        Some(("add", matches)) => {
            let key = b64::to_u64(matches.get_one::<String>("key").unwrap().as_str());
            let catagory = matches.get_one::<String>("catagory").unwrap();
            let location = matches.get_one::<String>("location").unwrap();
            let quantity = matches
                .get_one::<String>("quantity")
                .unwrap()
                .parse::<u64>()
                .unwrap();
            let fields: Vec<&String> = matches.get_many::<String>("FIELD").unwrap().collect();

            let mut entry = Entry::new(
                catagory,
                key,
                location,
                quantity,
                Local::now().timestamp(),
                Local::now().timestamp(),
            );

            for field in fields {
                entry.add_field(EntryField::from_str(field).unwrap());
            }

            println!("{}", entry);

            if confirm() == false {
                return;
            }

            db.add_entry(entry).unwrap();
        }
        Some(("import_csv", matches)) => {
            let file_name = matches.get_one::<String>("FILE").unwrap();

            let entries = csv::csv_to_entries(file_name).unwrap();

            for entry in entries {
                println!("{}", entry);
                if confirm() == false {
                    return;
                }
                db.add_entry(entry).unwrap();
            }
        }
        Some(("grab", matches)) => {
            let key = b64::to_u64(matches.get_one::<String>("KEY").unwrap().as_str());
            println!("{}", db.grab_entry(key).unwrap());
        }
        Some(("delete", matches)) => {
            let key = b64::to_u64(matches.get_one::<String>("KEY").unwrap().as_str());

            println!("{}", db.grab_entry(key).unwrap());

            if confirm() == false {
                return;
            }

            db.delete_entry(key).unwrap();
        }
        Some(("add_catagory", matches)) => {
            let fields: Vec<&String> = matches.get_many::<String>("FIELD").unwrap().collect();

            let catagory_id = matches.get_one::<String>("catagory").unwrap();

            let mut catagory = Catagory::new(catagory_id);

            for field in fields {
                catagory.add_field(CatagoryField::from_str(field).unwrap());
            }

            println!("{}", catagory);

            if confirm() == false {
                return;
            }

            db.add_catagory(catagory).unwrap();
        }
        Some(("list", matches)) => {
            let catagory = matches.get_one::<String>("catagory").unwrap();
            let fields: Vec<&str> = match matches.get_many::<String>("CONSTRAINTS") {
                Some(matches) => matches.map(|x| x.as_str()).collect(),
                None => vec!["KEY>=0"],
            };

            println!("Search Constraints: {:?}", fields);

            for entry in db.search_catagory(catagory, fields).unwrap() {
                println!("{}", entry);
            }
        }
        Some(("take", matches)) => {
            let key = b64::to_u64(matches.get_one::<String>("key").unwrap().as_str());
            let quantity = matches
                .get_one::<String>("QUANTITY")
                .unwrap()
                .parse::<u64>()
                .unwrap();

            let entry = db.grab_entry(key).unwrap();

            println!("{}\nCurrent Quantity:\t{}", &entry, entry.quantity);

            if quantity > entry.quantity {
                panic!("Trying to take a quantity bigger than that exists!")
            }

            let new_quantity: u64 = entry.quantity - quantity;

            println!("New Quantity:\t{}", new_quantity);

            if new_quantity == 0 {
                println!("New quantity is zero, wish to delete entry?");

                if confirm() == true {
                    db.delete_entry(key).unwrap();
                    return;
                }
            }

            println!("Commit new quantity?");

            if confirm() == false {
                return;
            }

            db.mod_entry(
                key,
                vec![EntryField::new("QUANTITY", &new_quantity.to_string())],
            )
            .unwrap();
        }
        Some(("give", matches)) => {
            let key = b64::to_u64(matches.get_one::<String>("key").unwrap().as_str());
            let quantity = matches
                .get_one::<String>("QUANTITY")
                .unwrap()
                .parse::<u64>()
                .unwrap();

            let entry = db.grab_entry(key).unwrap();

            println!("{}\nCurrent Quantity:\t{}", &entry, entry.quantity);

            let new_quantity: u64 = entry.quantity + quantity;

            println!("New Quantity:\t{}", new_quantity);

            println!("Commit new quantity?");

            if confirm() == false {
                return;
            }

            db.mod_entry(
                key,
                vec![EntryField::new("QUANTITY", &new_quantity.to_string())],
            )
            .unwrap();
        }
        Some(("fill_template", matches)) => {
            let in_filename = matches.get_one::<String>("IN").unwrap();
            let out_filename = matches.get_one::<String>("out").unwrap();

            let data = fs::read_to_string(in_filename).unwrap();

            fs::write(out_filename, db.fill_svg_template(data).unwrap()).unwrap();
        }
        _ => {}
    }
}
