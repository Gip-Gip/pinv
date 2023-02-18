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
}
