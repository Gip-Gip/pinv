use clap::{arg, Command, command};
use std::fs::File;
use std::fs;
use pinv::db::{Db, Catagory, CatagoryField};

fn main() {
    let mut db = Db::init();
    let matches = command!()
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(Command::new("add_catagory").args(&[
            arg!([CATAGORY]).required(true),
            arg!(-f --field <"NAME:TYPE"> ... "field_1").required(true),
        ]))
        .subcommand(Command::new("fill_template").args(&[
            arg!([IN] "template file").required(true),
            arg!(-o --out <FILE> "output file").required(true),
        ])).get_matches();

    match matches.subcommand() {
        Some(("add_catagory", matches)) => {
            let fields: Vec<&String> = matches.get_many::<String>("field").unwrap().collect();

            let catagory_id = matches.get_one::<String>("CATAGORY").unwrap();

            let mut catagory = Catagory::new(catagory_id);

            for field in fields {
                catagory.add_field(CatagoryField::from_str(field).unwrap());
            }

            println!("{}", catagory);

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
