//! Functions used for importing CSVs into the local pinv database.

use crate::b64;
use crate::db::{Entry, EntryField};
use chrono::Local;
use csv::ReaderBuilder;
use simple_error::bail;
use std::error::Error;

/// Take the name of a csv file and convert the rows in it to entries
pub fn csv_to_entries(file_name: &str) -> Result<Vec<Entry>, Box<dyn Error>> {
    let mut entries = Vec::<Entry>::new();
    let mut csv_reader = ReaderBuilder::new()
        .has_headers(false)
        .quoting(false)
        .delimiter(b'\x1E')
        .from_path(file_name)?;
    let mut csv_records = csv_reader.records();

    // First, get the catagory(first record)
    let catagory = match csv_records.next() {
        Some(result) => {
            let record = result?;

            record[0].to_owned().replace("'", "") // There should only be one column in a record
        }
        None => {
            bail!("No records in CSV!");
        }
    };

    eprintln!("{}", catagory);

    // Next, get the names of the fields
    let fields: Vec<String> = match csv_records.next() {
        Some(result) => result?
            .iter()
            .map(|field| field.to_uppercase().to_owned().replace("'", ""))
            .collect(),
        None => {
            bail!("Missing field definitions!");
        }
    };

    // All following rows in the csv are entries
    for result in csv_records {
        let record = result?;

        let mut key: Option<u64> = None;
        let mut quantity: Option<u64> = None;
        let mut location: Option<String> = None;
        let mut entry_fields = Vec::<String>::new();

        for (i, field) in record.iter().enumerate() {
            match fields[i].as_str() {
                "KEY" => {
                    key = Some(b64::to_u64(&field.replace("'", "")).unwrap());
                }
                "QUANTITY" => {
                    quantity = Some(field.parse::<u64>()?);
                }
                "LOCATION" => {
                    location = Some(field.to_owned().replace("'", ""));
                }
                _ => {
                    entry_fields.push(format!("{}={}", fields[i], field));
                }
            }
        }

        let key = key.expect("No key field provided!");
        let quantity = quantity.expect("No key field provided!");
        let location = location.expect("No location field provided!");

        let mut entry = Entry::new(
            &catagory,
            key,
            &location,
            quantity,
            Local::now().timestamp(),
            Local::now().timestamp(),
        );

        for field in entry_fields {
            let field = EntryField::from_str(&field)?;
            if field.value.len() > 0 {
                entry.add_field(field);
            }
        }

        entries.push(entry);
    }

    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use crate::db::Db;
    use serial_test::*;

    // Test reading the test.csv file
    #[test]
    #[serial]
    pub fn test_csv_to_entries() {
        let mut db = Db::_new_test();

        db.add_catagory(db::tests::test_catagory_a()).unwrap();

        for entry in csv_to_entries("test.csv").unwrap() {
            db.add_entry(entry).unwrap();
        }
    }
}
