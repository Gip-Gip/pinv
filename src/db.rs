//! Everything needed to interact with a pinv database

use crate::b64;
use chrono::{Local, TimeZone};
use core::fmt;
use directories::ProjectDirs;
use lazy_static::lazy_static;
use regex::Regex;
use rusqlite::Error as SqlError;
use rusqlite::{types::ValueRef, Connection, OptionalExtension};
use simple_error::bail;
use std::{cmp, error::Error, fs};

/// Datatypes in PINV
#[derive(Debug, Clone, PartialEq)]
pub enum DataType {
    /// Null, nothing
    NULL,
    /// Any whole number negative or non-negative
    INTEGER,
    /// Any number with a decimal in it
    REAL,
    /// Any unicode string
    TEXT,
    /// Raw data, currently not in use
    BLOB,
}

impl DataType {
    /// Get the code character of a certain datatype, like i for integer.
    pub fn get_char(&self) -> char {
        match self {
            Self::NULL => 'n',
            Self::INTEGER => 'i',
            Self::REAL => 'r',
            Self::TEXT => 't',
            Self::BLOB => 'b',
        }
    }

    /// Get the datatype from a char
    pub fn from_char(character: char) -> Result<Self, Box<dyn Error>> {
        Ok(match character {
            'n' => DataType::NULL,
            'i' => DataType::INTEGER,
            'r' => DataType::REAL,
            't' => DataType::TEXT,
            'b' => DataType::BLOB,
            _ => {
                bail!(r#"Invalid data type "{}"!"#, character);
            }
        })
    }
}

/// Datatypes in SQLite
pub enum SQLValue {
    /// Null, nothing
    NULL,
    /// Any whole number negative or non-negative
    INTEGER(u64),
    /// Any number with a decimal in it
    REAL(f64),
    /// Any unicode string
    TEXT(String),
    /// Raw data, currently not in use
    BLOB(Vec<u8>),
}

/// Used to define fields in catagories
#[derive(Debug, Clone, PartialEq)]
pub struct CatagoryField {
    /// id of the field, case insensitive
    pub id: String,
    /// pinv datatype of the field
    pub datatype: DataType,
}

impl CatagoryField {
    /// Create a new field from an id and a datatype
    pub fn new(id: &str, datatype: DataType) -> Self {
        let id = id.to_owned();

        Self { id, datatype }
    }

    /// Create a field from a string.
    ///
    /// Format is *id*:*datatype*, where id is the case-insensitive id of the
    /// field and datatype is the code character of a pinv datatype.
    ///
    /// Example,
    ///
    /// `max_volts:r`
    ///
    /// would create a field named "max_volts" of type real.
    pub fn from_str(string: &str) -> Result<Self, Box<dyn Error>> {
        // !TODO! Needs better code to detect if a string is valid or not
        let split_str: Vec<&str> = string.split(":").collect();

        // If the string was split more than once, or not at all, we got a problem!
        if split_str.len() != 2 {
            bail!(r#"Invalid field definition "{}"!"#, string);
        }

        let datatype = DataType::from_char(split_str[1].chars().next().unwrap())?;

        Ok(Self {
            id: split_str[0].to_owned().to_uppercase(), // Make it case insensitive by converting the id to uppercase
            datatype,
        })
    }

    /// Get the type of the field and convert it to it's SQL keyword
    /// equivalent. E.g. a field with type integer would return "INTEGER"
    pub fn sql_type(&self) -> String {
        match &self.datatype {
            DataType::NULL => "NULL".to_owned(),
            DataType::INTEGER => "INTEGER".to_owned(),
            DataType::REAL => "REAL".to_owned(),
            DataType::TEXT => "TEXT".to_owned(),
            DataType::BLOB => "BLOB".to_owned(),
        }
    }
}

/// Used to help define catagories(which are translated directly into sql tables)
#[derive(Debug, Clone, PartialEq)]
pub struct Catagory {
    /// ID of the catagory, case insensitive
    pub id: String,
    /// Fields associated with the catagory
    pub fields: Vec<CatagoryField>,
}

impl Catagory {
    /// Create an empty catagory with an id
    pub fn new(id: &str) -> Self {
        let fields = Vec::new();

        Self {
            id: id.to_owned().to_uppercase(), // Make it case insensitive by converting the id to uppercase
            fields,
        }
    }

    /// Create a catagory with an id and a vector of fields
    pub fn with_fields(id: &str, fields: Vec<CatagoryField>) -> Self {
        Self {
            id: id.to_owned().to_uppercase(), // Make it case insensitive by converting the id to uppercase
            fields,
        }
    }

    /// Add a field to the catagory
    pub fn add_field(&mut self, field: CatagoryField) {
        self.fields.push(field);
    }
}

impl fmt::Display for Catagory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Get the longest ID in all the fields
        let mut padlen: usize = 0;

        for field in &self.fields {
            padlen = cmp::max(padlen, field.id.len());
        }

        let mut out: String = format!("CATAGORY {}:", self.id);

        for field in &self.fields {
            out.push_str(
                format!(
                    "\n    {}:{foo: >padlen$} {}",
                    field.id,
                    field.sql_type(),
                    padlen = padlen - field.id.len(),
                    foo = ""
                )
                .as_str(),
            );
        }

        write!(f, "{}", out)
    }
}

/// Fields for entries
#[derive(Debug, Clone, PartialEq)]
pub struct EntryField {
    /// ID of the field, case insensitive.
    pub id: String,
    /// Value of the field, as a string(not yet parsed).
    pub value: String,
}

impl EntryField {
    /// Create  a new fields with an id and a value.
    pub fn new(id: &str, value: &str) -> Self {
        Self {
            id: id.to_owned(),
            value: value.to_owned(),
        }
    }

    /// Create an entry field from a string.
    ///
    /// Format is *id*=*value*, where id is the case-insensitive field id and
    /// value is the value you want to assign to the field.
    ///
    /// Example,
    ///
    /// `max_volts=3.3`
    ///
    /// Assigns the "max_volts" field a value of 3.3
    pub fn from_str(string: &str) -> Result<Self, Box<dyn Error>> {
        let split_str: Vec<&str> = string.split("=").collect();

        if split_str.len() != 2 {
            bail!("Invalid entry field definition '{}'!", string);
        }

        Ok(Self {
            id: split_str[0].to_owned().to_uppercase(),
            value: split_str[1].to_owned(),
        })
    }

    /// Get the value, formatted for sql
    ///
    pub fn get_sql(&self) -> String {
        self.value.clone()
    }
}

/// Used to create database entries
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    /// Catagory the entry belongs to
    pub catagory_id: String,
    /// Key of the catagory
    pub key: u64,
    /// Physical location of the entry
    pub location: String,
    /// Quantity of the entry
    pub quantity: u64,
    /// Creation time of the entry in unix time
    pub created: i64,
    /// Modification time of the entry in unix time
    pub modified: i64,
    /// Fields associated with the entry
    pub fields: Vec<EntryField>,
}

impl Entry {
    /// Create a new entry
    ///
    pub fn new(
        catagory_id: &str,
        key: u64,
        location: &str,
        quantity: u64,
        created: i64,
        modified: i64,
    ) -> Self {
        let fields = Vec::new();

        Self {
            catagory_id: catagory_id.to_owned().to_uppercase(), // Make the catagory id case
            // insensitive
            key,
            location: location.to_owned(),
            quantity,
            created,
            modified,
            fields,
        }
    }

    /// Add a field to the entry
    pub fn add_field(&mut self, field: EntryField) {
        self.fields.push(field);
    }

    /// Add a slice of fields to the entry
    pub fn add_fields(&mut self, fields: &[EntryField]) {
        self.fields.extend_from_slice(fields);
    }
}

impl fmt::Display for Entry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Get the longest ID in all the fields
        let mut padlen: usize = 8; // Set the max size to 8(the length of the "location" field)

        for field in &self.fields {
            padlen = cmp::max(padlen, field.id.len());
        }

        let created_str = Local.timestamp_opt(self.created, 0).unwrap().to_string();
        let modified_str = Local.timestamp_opt(self.modified, 0).unwrap().to_string();

        let mut out: String = format!(
            r#"ENTRY {}, CATAGORY {}:
    LOCATION{foo: >padlen$} = {},
    QUANTITY{foo: >padlen$} = {},
    CREATED {foo: >padlen$} = {},
    MODIFIED{foo: >padlen$} = {}"#,
            b64::from_u64(self.key),
            &self.catagory_id,
            &self.location,
            self.quantity,
            created_str,
            modified_str,
            padlen = padlen - 8,
            foo = ""
        );

        for field in &self.fields {
            out.push_str(
                format!(
                    ",\n    {}{foo: >padlen$} = {}",
                    field.id,
                    field.value,
                    padlen = padlen - field.id.len(),
                    foo = ""
                )
                .as_str(),
            );
        }

        write!(f, "{}", out)
    }
}

/// Used to interface with the pinv database. As of the current version, sqlite
/// is used to store and retrieve entries but this may change in the future.
pub struct Db {
    /// Connection to SQLite database
    pub connection: Connection,
}

impl Db {
    /// Initialize the pinv database. The database file is located in the
    /// current user's home data folder.
    pub fn init() -> Self {
        let qualifier = "org";
        let organisation = "Open Ape Shop";
        let application = "pinv";

        // Get the home data directories depending on the system
        let dirs = ProjectDirs::from(qualifier, organisation, application).unwrap();

        let data_dir = dirs.data_dir().to_owned();

        // Create the path to the datafile
        let mut db_filepath = data_dir.clone();
        db_filepath.push("pinv.db3");

        // If the data directory doesn't exist, create it
        // !TODO! Replace unwrap with proper error handling, perhaps
        if !data_dir.exists() {
            fs::create_dir_all(data_dir.as_path()).unwrap();
        }

        let connection = Connection::open(db_filepath).unwrap();

        // Check to see if the keys table exists in the database...
        // !TODO! use statement or something instead of a raw query, or maybe
        // just ditch raw sql entirely...
        let query = "SELECT name FROM sqlite_master WHERE type='table' AND name='KEYS'";

        match connection
            .query_row(query, [], |_| Ok(()))
            .optional()
            .unwrap()
        {
            Some(_) => {}
            None => {
                // In the case it doesn't exist, create it
                let query =
                    "CREATE TABLE KEYS (KEY INTEGER NOT NULL PRIMARY KEY, CATAGORY TEXT NOT NULL)";

                connection.execute(query, []).unwrap();
            }
        }

        Self { connection }
    }

    /// Create a database in RAM for testing purposes...
    pub fn _new_test() -> Self {
        let connection = Connection::open_in_memory().unwrap();

        // Add a key table to hold all keys we need to store

        let query = "CREATE TABLE KEYS (KEY INTEGER NOT NULL PRIMARY KEY, CATAGORY TEXT NOT NULL)";

        connection.execute(query, []).unwrap();

        Self { connection }
    }

    /// Add a key to the key table.
    fn add_key(&mut self, key: u64, catagory_id: &str) -> Result<(), Box<dyn Error>> {
        let query = format!(
            "INSERT INTO KEYS (KEY, CATAGORY)\nVALUES ({}, '{}')",
            key, catagory_id
        );

        self.connection.execute(&query, [])?;

        Ok(())
    }

    /// Swap a key for another in the key table
    fn swap_key(&mut self, old_key: u64, new_key: u64) -> Result<(), Box<dyn Error>> {
        let query = format!("UPDATE KEYS SET KEY={} WHERE KEY={}", new_key, old_key);

        self.connection.execute(&query, [])?;

        Ok(())
    }

    /// Add a catagory to the database.
    ///
    /// More or less just converts the catagory struct into an SQL table.
    pub fn add_catagory(&mut self, catagory: Catagory) -> Result<(), Box<dyn Error>> {
        // Verify the catagory won't cause any problems...
        Db::check_id_string(&catagory.id)?;

        // Check to see if the table exists first...
        let query = format!(
            "SELECT name FROM sqlite_master WHERE type='table' AND name='{}';",
            catagory.id
        );

        let query_result: Option<String> = self
            .connection
            .query_row(query.as_str(), [], |row| row.get(0))
            .optional()?;

        // If there was some result to the query, bail!
        match query_result {
            Some(_) => {
                bail!("Catagory already found in database!");
            }

            _ => {}
        }

        // Otherwise, add the catagory to the database
        let mut query = format!("CREATE TABLE {} (KEY INTEGER NOT NULL PRIMARY KEY, LOCATION TEXT NOT NULL, QUANTITY INTEGER NOT NULL, CREATED INTEGER NOT NULL, MODIFIED INTEGER NOT NULL, ", catagory.id);

        for (i, field) in catagory.fields.iter().enumerate() {
            // Verify that the field won't cause any problems...
            Db::check_id_string(&field.id)?;

            query.push_str(format!("{} {}", field.id, field.sql_type()).as_str());

            if i < catagory.fields.len() - 1 {
                query.push(',');
            }
        }

        query.push_str(")");

        self.connection.execute(&query, [])?;

        Ok(())
    }

    /// Add an entry to the database.
    ///
    /// More or less just converts the entry struct into SQL.
    pub fn add_entry(&mut self, entry: Entry) -> Result<(), Box<dyn Error>> {
        // Check and make sure the location is a valid string, and format it...
        let location =
            self.format_string_to_field(&entry.catagory_id, "LOCATION", &entry.location)?;
        let mut query_a = format!(
            "INSERT INTO {} (KEY, LOCATION, QUANTITY, CREATED, MODIFIED",
            entry.catagory_id
        );

        let mut query_b = format!(
            ")\nVALUES ({}, {}, {}, {}, {}",
            entry.key, location, entry.quantity, entry.created, entry.modified
        );

        for field in entry.fields {
            let field_id = field.id;
            let field_value =
                self.format_string_to_field(&entry.catagory_id, &field_id, &field.value)?;

            // Skip this field if the value is null
            if field_value.len() == 0 {
                continue;
            }
            // Verify they are valid names and types...
            Db::check_id_string(&field_id)?;

            query_a.push(',');
            query_b.push(',');
            query_a.push_str(&field_id);
            query_b.push_str(&field_value);
        }

        query_b.push(')');
        query_a.push_str(query_b.as_str());

        let query = query_a;

        // Add the key to the key table
        self.add_key(entry.key, &entry.catagory_id)?;

        match self.connection.execute(&query, []) {
            Ok(_) => Ok(()),
            Err(e) => {
                self.remove_key(entry.key).unwrap();

                Err(Box::new(e))
            }
        }
    }

    /// Get an entry from a query string
    pub fn query_to_entry(&self, query: &str, catagory_id: &str) -> Result<Entry, Box<dyn Error>> {
        let mut statement = self.connection.prepare(query)?;
        let mut column_names = Vec::<String>::new();

        for name in statement.column_names() {
            column_names.push(name.to_string())
        }

        // Assumes the key and other mandatory entry fields are in the same
        // column. Shouldn't change, right?
        Ok(statement.query_row([], |row| {
            let mut entry = Entry::new(
                catagory_id,
                row.get(0).unwrap(),
                (row.get::<usize, String>(1).unwrap()).as_str(),
                row.get(2).unwrap(),
                row.get(3).unwrap(),
                row.get(4).unwrap(),
            );

            // Get the rest of the fields
            let mut i: usize = 5;
            loop {
                let value: String = match row.get_ref(i) {
                    Ok(result) => format!("{}", Self::sqlval_to_string(result)),
                    Err(e) => match e {
                        // Break if we ran out of columns
                        SqlError::InvalidColumnIndex(_) => {
                            break;
                        }
                        // Otherwise, error
                        _ => {
                            return Err(e);
                        }
                    },
                };
                let entry_field = EntryField::new(&column_names[i], &value);
                entry.add_field(entry_field);

                i += 1;
            }

            Ok(entry)
        })?)
    }

    /// Get entries from a query
    pub fn query_to_entries(
        &self,
        query: &str,
        catagory_id: &str,
    ) -> Result<Vec<Entry>, Box<dyn Error>> {
        let mut statement = self.connection.prepare(query)?;
        let mut column_names = Vec::<String>::new();

        for name in statement.column_names() {
            column_names.push(name.to_string())
        }

        let mut rows = statement.query([])?;

        let mut entries = Vec::<Entry>::new();

        while let Some(row) = rows.next()? {
            let mut entry = Entry::new(
                catagory_id,
                row.get(0)?,
                (row.get::<usize, String>(1)?).as_str(),
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
            );

            let mut i: usize = 5;

            loop {
                let value: String = match row.get_ref(i) {
                    Ok(result) => format!("{}", Self::sqlval_to_string(result)),
                    Err(e) => match e {
                        SqlError::InvalidColumnIndex(_) => {
                            break;
                        }
                        _ => {
                            return Err(Box::new(e));
                        }
                    },
                };
                let entry_field = EntryField::new(&column_names[i], &value);
                entry.add_field(entry_field);

                i += 1;
            }

            entries.push(entry);
        }

        Ok(entries)
    }

    /// Grab the ids of the fields in a catagory.
    pub fn grab_catagory_fields(&self, name: &str) -> Result<Vec<String>, Box<dyn Error>> {
        let statement = self
            .connection
            .prepare(&format!("SELECT * FROM {}", name))?;
        let mut column_names = Vec::<String>::new();

        for name in statement.column_names() {
            column_names.push(name.to_string())
        }

        Ok(column_names)
    }

    /// Grab the types of the fields in a catagory.
    ///
    /// !TODO! Change the return type to the DataType enum.
    pub fn grab_catagory_types(&self, name: &str) -> Result<Vec<DataType>, Box<dyn Error>> {
        let mut statement = self
            .connection
            .prepare(&format!("PRAGMA table_info({})", name))?;

        let mut rows = statement.query([])?;
        let mut types = Vec::<DataType>::new();

        while let Some(row) = rows.next()? {
            let type_str: String = row.get(2)?;
            match type_str.as_str() {
                "INTEGER" => types.push(DataType::INTEGER),
                "REAL" => types.push(DataType::REAL),
                _ => types.push(DataType::TEXT),
            }
        }

        Ok(types)
    }

    /// Grab the catagory associated with a key.
    pub fn grab_catagory_from_key(&self, key: u64) -> Result<String, Box<dyn Error>> {
        let query = format!("SELECT CATAGORY FROM KEYS WHERE KEY={}", key);

        Ok(self.connection.query_row(&query, [], |row| row.get(0))?)
    }

    /// Grab an entry using only a key
    pub fn grab_entry(&self, key: u64) -> Result<Entry, Box<dyn Error>> {
        // First get the catagory the entry is in
        let catagory = self.grab_catagory_from_key(key)?;

        // Next grab the entry from the catagory
        let query = format!("SELECT * FROM {} WHERE KEY={}", catagory, key);

        self.query_to_entry(&query, &catagory)
    }

    /// Get the next unused key in the database
    pub fn grab_next_available_key(&self, key: u64) -> Result<u64, Box<dyn Error>> {
        // Prepare a statement where the key provided is seached for in the
        // key table
        let mut statement = self
            .connection
            .prepare("SELECT KEY FROM KEYS WHERE KEY = ?")?;

        let mut key = key;

        loop {
            match statement
                .query_row(rusqlite::params![key], |_| Ok(()))
                .optional()?
            {
                // If the key is in the table, increment the key and loop
                Some(_) => {}
                // Otherwise break and return the key
                None => {
                    break;
                }
            }

            key += 1
        }

        Ok(key)
    }

    /// Get all the catagories in the database.
    pub fn list_catagories(&self) -> Result<Vec<String>, Box<dyn Error>> {
        // Select all tables excluding the keys table
        let mut statement = self.connection.prepare(
            "SELECT name FROM sqlite_schema WHERE type='table' AND name!='KEYS' ORDER BY name;",
        )?;

        let mut rows = statement.query([])?;

        let mut names = Vec::<String>::new();

        while let Some(row) = rows.next()? {
            names.push(row.get(0)?);
        }

        Ok(names)
    }

    /// Get the stats of all catagories in the database. Currently only
    /// retrieves name and number of entries in a catagory.
    pub fn stat_catagories(&self) -> Result<Vec<Vec<String>>, Box<dyn Error>> {
        let catagories = self.list_catagories()?;

        let mut catagory_table = Vec::<Vec<String>>::with_capacity(catagories.len());

        for catagory in catagories {
            let count: usize = self.connection.query_row(
                &format!("SELECT COUNT(*) FROM {}", catagory),
                [],
                |row| row.get(0),
            )?;

            let catagory_row = vec![catagory, count.to_string()];

            catagory_table.push(catagory_row);
        }

        Ok(catagory_table)
    }

    /// Delete an entry given only the key
    pub fn delete_entry(&mut self, key: u64) -> Result<(), Box<dyn Error>> {
        // First, get the catagory the entry is in
        let catagory = self.grab_catagory_from_key(key)?;

        // Next delete the entry from the catagory
        let query = format!("DELETE FROM {} WHERE KEY={}", catagory, key);

        self.connection.execute(&query, [])?;

        // Delete the key
        self.remove_key(key).unwrap();
        Ok(())
    }

    /// Return entries in a catagory that match the given conditions
    pub fn search_catagory(
        &self,
        catagory_id: &str,
        conditions: &[String],
    ) -> Result<Vec<Entry>, Box<dyn Error>> {
        if conditions.len() == 0 {
            let query = format!("SELECT * FROM {}", catagory_id);

            return self.query_to_entries(&query, catagory_id);
        }

        let mut query = format!("SELECT * FROM {} WHERE ", catagory_id);

        for (i, condition) in conditions.iter().enumerate() {
            let condition_split: Vec<&str> = condition.split('=').collect();

            if condition_split.len() != 2 {
                bail!("Invalid condition \"{}\"!", condition);
            }

            let id = condition_split[0].to_uppercase();
            let value = condition_split[1];

            query.push_str(format!("{}={}", id, value).as_str());

            query.push_str(match i.cmp(&(conditions.len() - 1)) {
                cmp::Ordering::Less => " AND ",
                _ => ";",
            })
        }

        self.query_to_entries(&query, catagory_id)
    }

    /// Take an SVG template and fill it with all available keys
    pub fn fill_svg_template(&self, data: &str) -> Result<String, Box<dyn Error>> {
        let chunks: Vec<String> = data.split("FOO!").map(|chunk| chunk.to_owned()).collect();

        let mut data = String::new();
        let mut key = 0;

        for (i, chunk) in chunks.iter().enumerate() {
            data.push_str(&chunk);

            key = self.grab_next_available_key(key)?;
            if i < chunks.len() - 1 {
                data.push_str(&b64::from_u64(key));
            }

            key += 1
        }

        Ok(data)
    }

    /// Remove a key from the key table
    fn remove_key(&mut self, key: u64) -> Result<(), Box<dyn Error>> {
        let query = format!("DELETE FROM KEYS WHERE KEY={}", key);

        self.connection.execute(&query, [])?;

        Ok(())
    }

    /// Modify a entry with only a key and the fields to be modified
    pub fn mod_entry(&mut self, key: u64, fields: Vec<EntryField>) -> Result<(), Box<dyn Error>> {
        // First get the catagory the entry is in
        let catagory = self.grab_catagory_from_key(key)?;
        let mod_time_string = Local::now().timestamp().to_string();

        let mut fields_str = format!("MODIFIED={},", mod_time_string);

        let mut new_key: Option<u64> = Option::None;

        for (i, field) in fields.iter().enumerate() {
            // If the key is being modified, we need to update the key table
            let field_value = match field.id.as_str() {
                "KEY" => {
                    let field_value = b64::to_u64(&field.value)?;

                    new_key = Option::Some(field_value);
                    field_value.to_string()
                }
                // Otherise format the field
                _ => self.format_string_to_field(&catagory, &field.id, &field.value)?,
            };

            // Check and make sure the fields value is a-ok

            fields_str.push_str(&format!("{}={}", field.id, field_value));

            if i < fields.len() - 1 {
                fields_str.push(',')
            }
        }

        // Next update the entry
        let query = format!("UPDATE {} SET {} WHERE KEY={}", catagory, fields_str, key);

        // Swap the keys if a new key was specified
        if let Some(new_key) = new_key {
            self.swap_key(key, new_key)?;
        }

        match self.connection.execute(&query, []) {
            Ok(_) => Ok(()),
            Err(error) => {
                // Swap the keys back if there's an error!
                if let Some(new_key) = new_key {
                    self.swap_key(new_key, key)?;
                }

                Err(Box::new(error))
            }
        }
    }

    /// Convert an SQL valueref into a string
    fn sqlval_to_string(value: ValueRef) -> String {
        match value {
            ValueRef::Null => "NULL".to_owned(),
            ValueRef::Integer(i) => format!("{}", i),
            ValueRef::Real(f) => format!("{:e}", f),
            ValueRef::Text(s) => format!("{}", String::from_utf8_lossy(s)),
            ValueRef::Blob(_) => "BLOB".to_owned(),
        }
    }

    /// Check the valididy of an ID string and throw an error if not valid
    pub fn check_id_string(id: &str) -> Result<(), Box<dyn Error>> {
        lazy_static! {
            static ref VALID_RE: Regex =
                Regex::new(r#"\A[A-Z][\S&&[^a-z+\\\-*/%&|\^=><;]]*\z"#).unwrap();
        }

        match VALID_RE.is_match(id) {
            true => Ok(()),
            false => {
                bail!("{} is not a valid ID string!", id);
            }
        }
    }

    /// Check the valididy of a value string and throw an error if not valid
    pub fn check_value_string(value: &str, datatype: DataType) -> Result<(), Box<dyn Error>> {
        lazy_static! {
            static ref VALID_TEXT_PREP_RE: Regex = Regex::new(r#"\\'"#).unwrap();
            static ref VALID_TEXT_RE: Regex = Regex::new(r#"\A'[^']*'\z"#).unwrap();
            static ref VALID_INTEGER_PREP_RE: Regex = Regex::new(r#"e\d+"#).unwrap();
            static ref VALID_INTEGER_RE: Regex = Regex::new(r#"\A\-*\d+\z"#).unwrap();
            static ref VALID_REAL_PREP_RE: Regex = Regex::new(r#"\.\d+|e\d+|e\-\d"#).unwrap();
            static ref VALID_REAL_RE: Regex = VALID_INTEGER_RE.clone();
        }

        match datatype {
            DataType::TEXT => {
                let value = VALID_TEXT_PREP_RE.replace_all(value, "");

                match VALID_TEXT_RE.is_match(&value) {
                    true => Ok(()),
                    false => {
                        bail!("{} is not a valid text!", value);
                    }
                }
            }

            DataType::INTEGER => {
                let value = VALID_INTEGER_PREP_RE.replace_all(value, "");
                match VALID_INTEGER_RE.is_match(&value) {
                    true => Ok(()),
                    false => {
                        bail!("{} is not a valid integer!", value);
                    }
                }
            }

            DataType::REAL => {
                let value = VALID_REAL_PREP_RE.replace_all(value, "");
                match VALID_REAL_RE.is_match(&value) {
                    true => Ok(()),
                    false => {
                        bail!("{} is not a valid real!", value);
                    }
                }
            }

            _ => {
                bail!("Unsupported type!");
            }
        }
    }

    /// Format a string to be appropriate to the field it belongs to
    fn format_string_to_field(
        &self,
        catagory_id: &str,
        field_id: &str,
        field_value: &str,
    ) -> Result<String, Box<dyn Error>> {
        let datatype = self.field_type(catagory_id, field_id)?;

        let out = match datatype {
            DataType::TEXT => format!("'{}'", field_value),
            _ => field_value.to_string(),
        };

        Db::check_value_string(&out, datatype)?;

        Ok(out)
    }

    /// Get the type of a field
    pub fn field_type(
        &self,
        catagory_id: &str,
        field_id: &str,
    ) -> Result<DataType, Box<dyn Error>> {
        let fields = self.grab_catagory_fields(catagory_id)?;
        let types = self.grab_catagory_types(catagory_id)?;

        let i = match fields.iter().position(move |field| field == field_id) {
            Some(i) => i,
            None => {
                bail!("Field {} not found in {}!", field_id, catagory_id);
            }
        };

        Ok(types[i].clone())
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    // This test uses two template catagories and two template entries per
    // catagory. The catagories are to represent real life scenarios in which
    // I plan to use pinv for, so they should cover the base use cases
    //
    //  CATAGORY 'RESISTOR':
    //      MPN:        TEXT,
    //      MFCD_BY:    TEXT,
    //      OHMS:       REAL,
    //      WATTS:      REAL,
    //      TOLERANCE:  REAL,
    //      PPM_C:      REAL,
    //      TERM_STYLE: TEXT,
    //      MAKEUP:     TEXT,
    //      CASE_CODE:  TEXT,
    //      DATASHEET:  TEXT,
    //
    //  CATAGORY 'CAPACITOR':
    //      MPN:        TEXT,
    //      FARADS:     REAL,
    //      VOLTAGE_DC: REAL,
    //      VOLTAGE_AC: REAL,
    //      HOURS:      REAL,
    //      TOLERANCE:  REAL,
    //      TERM_STYLE: TEXT,
    //      MAKEUP:     TEXT,
    //      CASE_CODE:  TEXT,
    //      DATASHEET:  TEXT,
    //
    //
    //
    //  ENTRY 0, CATAGORY 'RESISTOR':
    //      KEY         = 0
    //      LOCATION    = 'bazville'
    //      QUANTITY    = 10
    //      MPN         = 'ERJ-PM8F8204V'
    //      MFCD_BY     = 'Panasonic'
    //      OHMS        = 8.2e6
    //      WATTS       = 660e-3
    //      TOLERANCE   = 0.01
    //      PPM_C       = 100
    //      TERM_STYLE  = 'SMD'
    //      MAKEUP      = 'Thick Film'
    //      CASE_CODE   = '1206'
    //      DATASHEET   = 'https://www.mouser.com/datasheet/2/315/Panasonic_Resistor_ERJ_P_PA_PM_Series_022422-2933625.pdf'
    //
    //  ENTRY 1, CATAGORY 'RESISTOR':
    //      KEY         = 1
    //      LOCATION    = 'bazville'
    //      QUANTITY    = 2
    //      MPN         = 'HPCR0819AK39RST'
    //      MFCD_BY     = 'TE Connectivity/Holsworthy'
    //      OHMS        = 39
    //      WATTS       = 2
    //      TOLERANCE   = 0.1
    //      PPM_C       = -800
    //      TERM_STYLE  = 'Through Hole'
    //      MAKEUP      = 'Ceramic Comp'
    //      CASE_CODE   = '19.1x7.9 Axial'
    //      DATASHEET   = 'https://www.mouser.com/datasheet/2/418/8/ENG_DS_1773193_1_B-2888555.pdf'
    //
    //  ENTRY 2, CATAGORY 'CAPACITOR':
    //      KEY         = 2
    //      LOCATION    = 'bartown'
    //      QUANTITY    = 21
    //      MPN         = 'MAL217250122E3'
    //      MFCD_BY     = 'Vishay'
    //      FARADS      = 1.2e-3
    //      VOLTAGE_DC  = 35
    //      TOLERANCE   = 0.2
    //      TERM_STYLE  = 'Through Hole'
    //      MAKEUP      = 'Aluminum Electrolytic'
    //      CASE_CODE   = '25x12.5 Radial'
    //      DATASHEET   = 'https://www.vishay.com/doc?28499'
    //
    //  Entry 3, CATAGORY 'CAPACITOR':
    //      KEY         = 3
    //      LOCATION    = 'bartown'
    //      QUANTITY    = 100
    //      MPN         = 'MC08EA220J-TF'
    //      MFCD_BY     = 'Cornell Dubilier - CDE'
    //      FARADS      = 22e-12
    //      VOLTAGE_DC  = 100
    //      VOLTAGE_AC  = 70
    //      TOLERANCE   = 0.05
    //      TERM_STYLE  = 'SMD'
    //      MAKEUP      = 'Mica'
    //      CASE_CODE   = '0805'
    //      DATASHEET   = 'https://www.mouser.com/datasheet/2/88/CDUB_S_A0011956908_1-2540249.pdf'

    // Return test catagory a, 'RESISTOR'
    pub fn test_catagory_a() -> Catagory {
        Catagory {
            id: "RESISTOR".to_owned(),
            fields: vec![
                CatagoryField {
                    id: "MPN".to_owned(),
                    datatype: DataType::TEXT,
                },
                CatagoryField {
                    id: "MFCD_BY".to_owned(),
                    datatype: DataType::TEXT,
                },
                CatagoryField {
                    id: "OHMS".to_owned(),
                    datatype: DataType::REAL,
                },
                CatagoryField {
                    id: "WATTS".to_owned(),
                    datatype: DataType::REAL,
                },
                CatagoryField {
                    id: "TOLERANCE".to_owned(),
                    datatype: DataType::REAL,
                },
                CatagoryField {
                    id: "PPM_C".to_owned(),
                    datatype: DataType::REAL,
                },
                CatagoryField {
                    id: "TERM_STYLE".to_owned(),
                    datatype: DataType::TEXT,
                },
                CatagoryField {
                    id: "MAKEUP".to_owned(),
                    datatype: DataType::TEXT,
                },
                CatagoryField {
                    id: "CASE_CODE".to_owned(),
                    datatype: DataType::TEXT,
                },
                CatagoryField {
                    id: "DATASHEET".to_owned(),
                    datatype: DataType::TEXT,
                },
            ],
        }
    }

    // Return test catagory b, 'CAPACITOR'
    pub fn test_catagory_b() -> Catagory {
        Catagory {
            id: "CAPACITOR".to_owned(),
            fields: vec![
                CatagoryField {
                    id: "MPN".to_owned(),
                    datatype: DataType::TEXT,
                },
                CatagoryField {
                    id: "MFCD_BY".to_owned(),
                    datatype: DataType::TEXT,
                },
                CatagoryField {
                    id: "FARADS".to_owned(),
                    datatype: DataType::REAL,
                },
                CatagoryField {
                    id: "VOLTAGE_DC".to_owned(),
                    datatype: DataType::REAL,
                },
                CatagoryField {
                    id: "VOLTAGE_AC".to_owned(),
                    datatype: DataType::REAL,
                },
                CatagoryField {
                    id: "HOURS".to_owned(),
                    datatype: DataType::REAL,
                },
                CatagoryField {
                    id: "TOLERANCE".to_owned(),
                    datatype: DataType::REAL,
                },
                CatagoryField {
                    id: "TERM_STYLE".to_owned(),
                    datatype: DataType::TEXT,
                },
                CatagoryField {
                    id: "MAKEUP".to_owned(),
                    datatype: DataType::TEXT,
                },
                CatagoryField {
                    id: "CASE_CODE".to_owned(),
                    datatype: DataType::TEXT,
                },
                CatagoryField {
                    id: "DATASHEET".to_owned(),
                    datatype: DataType::TEXT,
                },
            ],
        }
    }

    // Test entry 0
    pub fn test_entry_0() -> Entry {
        Entry {
            catagory_id: "RESISTOR".to_owned(),
            key: 0,
            location: "bazville".to_owned(),
            quantity: 10,
            created: 0,
            modified: 0,
            fields: vec![
                EntryField{
                    id: "MPN".to_owned(),
                    value: "ERJ-PM8F8204V".to_owned()
                },
                EntryField {
                    id: "MFCD_BY".to_owned(),
                    value: "Panasonic".to_owned()
                },
                EntryField {
                    id: "OHMS".to_owned(),
                    value: "8.2e6".to_owned()
                },
                EntryField {
                    id: "WATTS".to_owned(),
                    value: "6.6e-1".to_owned()
                },
                EntryField {
                    id: "TOLERANCE".to_owned(),
                    value: "1e-2".to_owned()
                },
                EntryField {
                    id: "PPM_C".to_owned(),
                    value: "1e2".to_owned()
                },
                EntryField {
                    id: "TERM_STYLE".to_owned(),
                    value: "SMD".to_owned()
                },
                EntryField {
                    id: "MAKEUP".to_owned(),
                    value: "Thick Film".to_owned()
                },
                EntryField {
                    id: "CASE_CODE".to_owned(),
                    value: "1206".to_owned()
                },
                EntryField {
                    id: "DATASHEET".to_owned(),
                    value: "https://www.mouser.com/datasheet/2/315/Panasonic_Resistor_ERJ_P_PA_PM_Series_022422-2933625.pdf".to_owned()
                }
            ]
        }
    }

    // Test entry 1
    pub fn test_entry_1() -> Entry {
        Entry {
            catagory_id: "RESISTOR".to_owned(),
            key: 1,
            location: "bazville".to_owned(),
            quantity: 2,
            created: 0,
            modified: 0,
            fields: vec![
                EntryField {
                    id: "MPN".to_owned(),
                    value: "HPCR0819AK39RST".to_owned(),
                },
                EntryField {
                    id: "MFCD_BY".to_owned(),
                    value: "TE Connectivity/Holsworthy".to_owned(),
                },
                EntryField {
                    id: "OHMS".to_owned(),
                    value: "3.9e1".to_owned(),
                },
                EntryField {
                    id: "WATTS".to_owned(),
                    value: "2e0".to_owned(),
                },
                EntryField {
                    id: "TOLERANCE".to_owned(),
                    value: "1e1".to_owned(),
                },
                EntryField {
                    id: "PPM_C".to_owned(),
                    value: "-8e2".to_owned(),
                },
                EntryField {
                    id: "TERM_STYLE".to_owned(),
                    value: "Through Hole".to_owned(),
                },
                EntryField {
                    id: "MAKEUP".to_owned(),
                    value: "Ceramic Comp".to_owned(),
                },
                EntryField {
                    id: "CASE_CODE".to_owned(),
                    value: "19.1x7.9 Axial".to_owned(),
                },
                EntryField {
                    id: "DATASHEET".to_owned(),
                    value:
                        "https://www.mouser.com/datasheet/2/418/8/ENG_DS_1773193_1_B-2888555.pdf"
                            .to_owned(),
                },
            ],
        }
    }

    // Test entry 2
    pub fn test_entry_2() -> Entry {
        Entry {
            catagory_id: "CAPACITOR".to_owned(),
            key: 2,
            location: "barville".to_owned(),
            quantity: 21,
            created: 0,
            modified: 0,
            fields: vec![
                EntryField {
                    id: "MPN".to_owned(),
                    value: "HPCR0819AK39RST".to_owned(),
                },
                EntryField {
                    id: "MFCD_BY".to_owned(),
                    value: "Vishay".to_owned(),
                },
                EntryField {
                    id: "FARADS".to_owned(),
                    value: "1.2e-3".to_owned(),
                },
                EntryField {
                    id: "VOLTAGE_DC".to_owned(),
                    value: "3.5e1".to_owned(),
                },
                EntryField {
                    id: "TOLERANCE".to_owned(),
                    value: "2e-1".to_owned(),
                },
                EntryField {
                    id: "TERM_STYLE".to_owned(),
                    value: "Through Hole".to_owned(),
                },
                EntryField {
                    id: "MAKEUP".to_owned(),
                    value: "Aluminum Electrolytic".to_owned(),
                },
                EntryField {
                    id: "CASE_CODE".to_owned(),
                    value: "25x12.5 Radial".to_owned(),
                },
                EntryField {
                    id: "DATASHEET".to_owned(),
                    value: "https://www.vishay.com/doc?28499".to_owned(),
                },
            ],
        }
    }

    // Test entry 3
    pub fn test_entry_3() -> Entry {
        Entry {
            catagory_id: "CAPACITOR".to_owned(),
            key: 3,
            location: "barville".to_owned(),
            quantity: 100,
            created: 0,
            modified: 0,
            fields: vec![
                EntryField {
                    id: "MPN".to_owned(),
                    value: "MC08EA220J-TF".to_owned(),
                },
                EntryField {
                    id: "MFCD_BY".to_owned(),
                    value: "Cornell Dubilier - CDE".to_owned(),
                },
                EntryField {
                    id: "FARADS".to_owned(),
                    value: "2.2e-13".to_owned(),
                },
                EntryField {
                    id: "VOLTAGE_DC".to_owned(),
                    value: "1e2".to_owned(),
                },
                EntryField {
                    id: "VOLTAGE_AC".to_owned(),
                    value: "7e1".to_owned(),
                },
                EntryField {
                    id: "TOLERANCE".to_owned(),
                    value: "5e-2".to_owned(),
                },
                EntryField {
                    id: "TERM_STYLE".to_owned(),
                    value: "SMD".to_owned(),
                },
                EntryField {
                    id: "MAKEUP".to_owned(),
                    value: "Mica".to_owned(),
                },
                EntryField {
                    id: "CASE_CODE".to_owned(),
                    value: "0805".to_owned(),
                },
                EntryField {
                    id: "DATASHEET".to_owned(),
                    value: "https://www.mouser.com/datasheet/2/88/CDUB_S_A0011956908_1-2540249.pdf"
                        .to_owned(),
                },
            ],
        }
    }

    // Test creating a field from a string
    #[test]
    fn test_db_create_field_from_string() {
        let field = CatagoryField::from_str("foo:i").unwrap();

        assert_eq!(field.datatype, DataType::INTEGER);
    }

    // Test getting the sql type from a field
    #[test]
    fn test_db_get_sql_type() {
        let field = CatagoryField::from_str("foo:i").unwrap();

        assert_eq!(field.sql_type(), "INTEGER");
    }

    // Test creating a catagory
    #[test]
    fn test_db_new_catagory() {
        let mut catagory = Catagory::new("resistor");

        catagory.add_field(CatagoryField::from_str("mpn:t").unwrap());
        catagory.add_field(CatagoryField::from_str("mfcd_by:t").unwrap());
        catagory.add_field(CatagoryField::from_str("ohms:r").unwrap());
        catagory.add_field(CatagoryField::from_str("watts:r").unwrap());
        catagory.add_field(CatagoryField::from_str("tolerance:r").unwrap());
        catagory.add_field(CatagoryField::from_str("ppm_c:r").unwrap());
        catagory.add_field(CatagoryField::from_str("term_style:t").unwrap());
        catagory.add_field(CatagoryField::from_str("makeup:t").unwrap());
        catagory.add_field(CatagoryField::from_str("case_code:t").unwrap());
        catagory.add_field(CatagoryField::from_str("datasheet:t").unwrap());

        assert_eq!(catagory, test_catagory_a());
    }
    // Test adding a catagory to a database
    #[test]
    fn test_db_add_catagory() {
        let mut db = Db::_new_test();
        let catagory_a = test_catagory_a();
        let catagory_b = test_catagory_b();

        // Shouldn't fail the first time
        db.add_catagory(catagory_a.clone()).unwrap();

        // Should fail the second time
        db.add_catagory(catagory_a).unwrap_err();

        // Test adding multiple catagories
        db.add_catagory(catagory_b).unwrap();
    }

    // Test creating an entry
    #[test]
    fn test_db_new_entry() {
        let mut entry = Entry::new("resistor", 0, "bazville", 10, 0, 0);

        entry.add_field(EntryField::from_str("mpn=ERJ-PM8F8204V").unwrap());
        entry.add_field(EntryField::from_str("mfcd_by=Panasonic").unwrap());
        entry.add_field(EntryField::from_str("ohms=8.2e6").unwrap());
        entry.add_field(EntryField::from_str("watts=6.6e-1").unwrap());
        entry.add_field(EntryField::from_str("tolerance=1e-2").unwrap());
        entry.add_field(EntryField::from_str("ppm_c=1e2").unwrap());
        entry.add_field(EntryField::from_str("term_style=SMD").unwrap());
        entry.add_field(EntryField::from_str("makeup=Thick Film").unwrap());
        entry.add_field(EntryField::from_str("case_code=1206").unwrap());
        entry.add_field(EntryField::from_str("datasheet=https://www.mouser.com/datasheet/2/315/Panasonic_Resistor_ERJ_P_PA_PM_Series_022422-2933625.pdf").unwrap());

        assert_eq!(entry, test_entry_0());
    }
    #[test]
    fn test_db_add_entry() {
        let mut db = Db::_new_test();

        db.add_catagory(test_catagory_b()).unwrap();

        // test entry 0 belongs to catagory a, so this should return an error
        db.add_entry(test_entry_0()).unwrap_err();

        // add catagory a and expect no errors
        db.add_catagory(test_catagory_a()).unwrap();
        db.add_entry(test_entry_0()).unwrap();

        // try to add the same entry again and expect an error
        db.add_entry(test_entry_0()).unwrap_err();

        // add a new entry and expect no errors
        db.add_entry(test_entry_1()).unwrap();

        // Just change the catagory on the entry 0 and expect an error
        let mut entry_0 = test_entry_0();

        entry_0.catagory_id = "CAPACITOR".to_owned();

        db.add_entry(entry_0).unwrap_err();

        // Add a different type of entry to a different catagory
        db.add_entry(test_entry_2()).unwrap();

        // Add another entry to catagory b
        db.add_entry(test_entry_3()).unwrap();

        // Change the key to one that exists in another catagory, expect an error
        let mut entry_3 = test_entry_3();

        entry_3.key = 0;

        db.add_entry(entry_3).unwrap_err();
    }

    #[test]
    fn test_db_grab_by_key() {
        let mut db = Db::_new_test();

        // Should fail
        db.grab_entry(0).unwrap_err();
        // Add the catagories and entries we need
        db.add_catagory(test_catagory_a()).unwrap();
        db.add_catagory(test_catagory_b()).unwrap();

        // Should still fail
        db.grab_entry(0).unwrap_err();

        db.add_entry(test_entry_0()).unwrap();
        db.add_entry(test_entry_1()).unwrap();

        // Grab the first entry by it's key

        let entry_0 = db.grab_entry(0).unwrap();

        assert_eq!(entry_0, test_entry_0());
    }

    #[test]
    fn test_db_delete_by_key() {
        let mut db = Db::_new_test();

        // Should fail
        db.delete_entry(0).unwrap_err();
        // Add catagories and entries needed
        db.add_catagory(test_catagory_a()).unwrap();
        db.add_catagory(test_catagory_b()).unwrap();

        // Should still fail
        db.delete_entry(0).unwrap_err();

        db.add_entry(test_entry_0()).unwrap();
        db.add_entry(test_entry_1()).unwrap();

        // Delete the first entry
        db.delete_entry(0).unwrap();

        // Try to grab it(should fail!)
        db.grab_entry(0).unwrap_err();
    }

    #[test]
    fn test_db_format_entry() {
        // Entries should be formatted a certian way, alike the comments above
        //
        //  ENTRY {key}, CATAGORY {catagory}:
        //      LOCATION     = {location},
        //      QUANTITY     = {quantity},
        //      {field_1_id} = {field_1_val},
        //      {field_2_id} = {field_2_val},
        //      ...
        //      {field_x_id} = {field_x_val}

        let test_string: String = format!(
            r#"ENTRY 0, CATAGORY RESISTOR:
    LOCATION   = bazville,
    QUANTITY   = 10,
    CREATED    = {time},
    MODIFIED   = {time},
    MPN        = ERJ-PM8F8204V,
    MFCD_BY    = Panasonic,
    OHMS       = 8.2e6,
    WATTS      = 6.6e-1,
    TOLERANCE  = 1e-2,
    PPM_C      = 1e2,
    TERM_STYLE = SMD,
    MAKEUP     = Thick Film,
    CASE_CODE  = 1206,
    DATASHEET  = https://www.mouser.com/datasheet/2/315/Panasonic_Resistor_ERJ_P_PA_PM_Series_022422-2933625.pdf"#,
            time = Local.timestamp_opt(0, 0).unwrap().to_string()
        );

        assert_eq!(test_string, format!("{}", test_entry_0()));
    }

    #[test]
    fn test_db_search_catagory() {
        let mut db = Db::_new_test();

        db.add_catagory(test_catagory_a()).unwrap();

        db.add_entry(test_entry_0()).unwrap();
        db.add_entry(test_entry_1()).unwrap();

        assert_eq!(
            db.search_catagory("RESISTOR", &vec!["ohms=8.2e6".to_string()])
                .unwrap()[0],
            test_entry_0()
        );
    }

    #[test]
    fn test_db_get_catagory_fields() {
        let mut db = Db::_new_test();

        db.add_catagory(test_catagory_a()).unwrap();

        assert_eq!(
            db.grab_catagory_fields("RESISTOR").unwrap(),
            vec![
                "KEY",
                "LOCATION",
                "QUANTITY",
                "CREATED",
                "MODIFIED",
                "MPN",
                "MFCD_BY",
                "OHMS",
                "WATTS",
                "TOLERANCE",
                "PPM_C",
                "TERM_STYLE",
                "MAKEUP",
                "CASE_CODE",
                "DATASHEET"
            ]
        );
    }

    #[test]
    fn test_db_modify_entry() {
        let mut db = Db::_new_test();

        db.add_catagory(test_catagory_a()).unwrap();

        db.add_entry(test_entry_0()).unwrap();

        assert_eq!(db.grab_entry(0).unwrap(), test_entry_0());

        db.mod_entry(0, vec![EntryField::from_str("quantity=9").unwrap()])
            .unwrap();

        assert_eq!(db.grab_entry(0).unwrap().quantity, 9);

        // Make sure modifying the key also works as expected...
        db.mod_entry(0, vec![EntryField::from_str("key=1").unwrap()])
            .unwrap();

        // Should fail...
        db.grab_entry(0).unwrap_err();
        // Shouldn't fail...
        db.grab_entry(1).unwrap();
    }

    #[test]
    fn test_db_string_format_id_test() {
        let good_id_1 = "FOO";
        let good_id_2 = "FOO0";

        let bad_id_1 = "FOO+";
        let bad_id_2 = "foo";
        let bad_id_3 = "0foo";

        // Should pass
        Db::check_id_string(good_id_1).unwrap();
        Db::check_id_string(good_id_2).unwrap();

        // Should fail
        Db::check_id_string(bad_id_1).unwrap_err();
        Db::check_id_string(bad_id_2).unwrap_err();
        Db::check_id_string(bad_id_3).unwrap_err();
    }

    #[test]
    fn test_db_string_format_value_test() {
        let good_string_1 = "'foo'";
        let good_string_2 = "'f\\'oo'";
        let good_number_1 = "123456789";
        let good_number_2 = "1e3";
        let good_float_1 = "1.2";

        let bad_string_1 = "'f'oo'";
        let bad_string_2 = "foo";
        let bad_number_1 = "e1";
        let bad_number_2 = "1fooga";
        let bad_number_3 = "1.0";

        // Should pass
        Db::check_value_string(good_string_1, DataType::TEXT).unwrap();
        Db::check_value_string(good_string_2, DataType::TEXT).unwrap();
        Db::check_value_string(good_number_1, DataType::INTEGER).unwrap();
        Db::check_value_string(good_number_1, DataType::REAL).unwrap();
        Db::check_value_string(good_number_2, DataType::INTEGER).unwrap();
        Db::check_value_string(good_number_2, DataType::REAL).unwrap();
        Db::check_value_string(good_float_1, DataType::REAL).unwrap();

        // Should fail
        Db::check_value_string(bad_string_1, DataType::TEXT).unwrap_err();
        Db::check_value_string(bad_string_2, DataType::TEXT).unwrap_err();
        Db::check_value_string(bad_number_1, DataType::INTEGER).unwrap_err();
        Db::check_value_string(bad_number_1, DataType::REAL).unwrap_err();
        Db::check_value_string(bad_number_2, DataType::INTEGER).unwrap_err();
        Db::check_value_string(bad_number_2, DataType::REAL).unwrap_err();
        Db::check_value_string(bad_number_3, DataType::INTEGER).unwrap_err();
    }
}
