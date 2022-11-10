use clap::{arg, Command, command};
use std::fs::File;
use std::fs;
use std::io::stdin;
use pinv::db::{Db, Catagory, CatagoryField, Entry, EntryField};
use pinv::b64;
use chrono::Local;

fn confirm() -> bool {
    println!("Confirm?(y/n)");

    let mut answer = String::new();

    stdin().read_line(&mut answer).unwrap();

    if answer.trim() == "y" {
        return true
    }
    false
}

fn main() {
    let mut db = Db::init();

    let matches = command!()
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(Command::new("add").args(&[
            arg!(-k --key <KEY> "entry key").required(true),
            arg!(-c --catagory <CATAGORY> "catagory to insert the entry in").required(true),
            arg!(-l --location <LOCATION> "location the entry will be stored in").required(true),
            arg!(-q --quantity <QUANTITY> "quantity of the entry to be stored").required(true),
            arg!([FIELD] ... "foo")
        ]))
        .subcommand(Command::new("grab").args(&[
            arg!([KEY] "key of the entry to be grabbed").required(true)
        ]))
        .subcommand(Command::new("add_catagory").args(&[
            arg!([CATAGORY]).required(true),
            arg!(-f --field <"NAME:TYPE"> ... "foo").required(true),
        ]))
        .subcommand(Command::new("fill_template").args(&[
            arg!([IN] "template file").required(true),
            arg!(-o --out <FILE> "output file").required(true),
        ])).get_matches();

    match matches.subcommand() {
        Some(("add", matches)) => {
            let key = b64::to_u64(matches.get_one::<String>("key").unwrap().as_str());
            let catagory = matches.get_one::<String>("catagory").unwrap();
            let location = matches.get_one::<String>("location").unwrap();
            let quantity = matches.get_one::<String>("quantity").unwrap().parse::<u64>().unwrap();
            let fields: Vec<&String> = matches.get_many::<String>("FIELD").unwrap().collect();

            let mut entry = Entry::new(catagory, key, location, quantity, Local::now().timestamp(), Local::now().timestamp());

            for field in fields {
                entry.add_field(EntryField::from_str(field).unwrap());
            }

            println!("{}", entry);

            if confirm() == false {
                return
            }
            
            db.add_entry(entry).unwrap();
        }
        Some(("grab", matches)) => {
            let key = b64::to_u64(matches.get_one::<String>("KEY").unwrap().as_str());

            println!("{}", db.grab_entry(key).unwrap());
        }
        Some(("add_catagory", matches)) => {
            let fields: Vec<&String> = matches.get_many::<String>("field").unwrap().collect();

            let catagory_id = matches.get_one::<String>("CATAGORY").unwrap();

            let mut catagory = Catagory::new(catagory_id);

            for field in fields {
                catagory.add_field(CatagoryField::from_str(field).unwrap());
            }

            println!("{}", catagory);

            if confirm() == false {
                return
            }

            db.add_catagory(catagory).unwrap();
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
