use chrono::{Local, TimeZone};
use rusqlite::{Connection, OptionalExtension, types::ValueRef};
use simple_error::bail;
use core::fmt;
use std::{error::Error, cmp, fmt::format, fs};
use rusqlite::Error as SqlError;
use directories::ProjectDirs;
use crate::b64;

/// Mapping to SQLite datatypes
#[derive(Debug, Clone, PartialEq)]
pub enum DataType {
    NULL,
    INTEGER,
    REAL,
    TEXT,
    BLOB,
}

pub enum SQLValue {
    NULL,
    INTEGER(u64),
    REAL(f64),
    TEXT(String),
    BLOB(Vec<u8>),
}

/// Used to define fields in catagories
#[derive(Debug, Clone, PartialEq)]
pub struct CatagoryField {
    pub id: String,
    pub data_type: DataType,
}

impl CatagoryField {
    pub fn from_str(string: &str) -> Result<Self, Box<dyn Error>> {
        let split_str: Vec<&str> = string.split(":").collect();

        // If the string was split more than once, or not at all, we got a problem!
        if split_str.len() != 2 {
            bail!(r#"Invalid field definition "{}"!"#, string);
        }

        let data_type = match split_str[1] {
            "n" => DataType::NULL,
            "i" => DataType::INTEGER,
            "r" => DataType::REAL,
            "t" => DataType::TEXT,
            "b" => DataType::BLOB,
            _ => {
                bail!(r#"Invalid data type "{}"!"#, split_str[1]);
            }
        };

        Ok(Self {
            id: split_str[0].to_owned().to_uppercase(), // Make it case insensitive by converting the id to uppercase
            data_type: data_type,
        })
    }

    pub fn sql_type(&self) -> String {
        match &self.data_type {
            DataType::NULL => "NULL".to_owned(),
            DataType::INTEGER => "INTEGER".to_owned(),
            DataType::REAL => "REAL".to_owned(),
            DataType::TEXT => "TEXT".to_owned(),
            DataType::BLOB => "BLOB".to_owned(),
        }
    }

    pub fn get_sql(&self) -> String {
        todo!()
    }
}

/// Used to help define catagories(which are translated directly into sql tables)
#[derive(Debug, Clone, PartialEq)]
pub struct Catagory {
    pub id: String,
    pub fields: Vec<CatagoryField>,
}

impl Catagory {
    pub fn new(id: &str) -> Self {
        let fields = Vec::new();

        Self {
            id: id.to_owned().to_uppercase(), // Make it case insensitive by converting the id to uppercase
            fields: fields,
        }
    }

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
            out.push_str(format!("\n    {}:{foo: >padlen$} {}", field.id, field.sql_type(), padlen = padlen-field.id.len(), foo="").as_str());
        }

        write!(f, "{}", out)
    }

}

/// Fields for entries
#[derive(Debug, Clone, PartialEq)]
pub struct EntryField {
    pub id: String,
    pub value: String,
}

impl EntryField {
    pub fn new(id: &str, value: &str) -> Self {
        Self {
            id: id.to_owned(),
            value: value.to_owned()
        }
    }
    /// Create an entry field from a string
    ///
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
#[derive(Debug, PartialEq)]
pub struct Entry {
    pub catagory_id: String,
    pub key: u64,
    pub location: String,
    pub quantity: u64,
    pub created: i64,
    pub modified: i64,
    pub fields: Vec<EntryField>,
}

impl Entry {
    /// Create a new entry
    ///
    pub fn new(catagory_id: &str, key: u64, location: &str, quantity: u64, created: i64, modified: i64) -> Self {
        let fields = Vec::new();

        Self {
            catagory_id: catagory_id.to_owned().to_uppercase(),
            key: key,
            location: location.to_owned(),
            quantity: quantity,
            created: created,
            modified: modified,
            fields: fields
        }
    }


    /// Add an entry field to the entry
    ///
    pub fn add_field(&mut self, field: EntryField) {
        self.fields.push(field);
    }
}

impl fmt::Display for Entry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Get the longest ID in all the fields
        let mut padlen: usize = 8; // Set the max size to 8(the length of the "location" field)

        for field in &self.fields {
            padlen = cmp::max(padlen, field.id.len());
        }

        let created_str = Local.timestamp(self.created, 0).to_string();
        let modified_str = Local.timestamp(self.modified, 0).to_string();

        let mut out: String = format!(r#"ENTRY {}, CATAGORY {}:
    LOCATION{foo: >padlen$} = '{}',
    QUANTITY{foo: >padlen$} = {},
    CREATED {foo: >padlen$} = {},
    MODIFIED{foo: >padlen$} = {}"#,
    b64::from_u64(self.key),
    &self.catagory_id,
    &self.location,
    self.quantity,
    created_str,
    modified_str,
    padlen = padlen-8, foo="");

        for field in &self.fields {
            out.push_str(format!(",\n    {}{foo: >padlen$} = {}", field.id, field.value, padlen = padlen-field.id.len(), foo="").as_str());
        }

        write!(f, "{}", out)
    }
}

/// Used to interface with an SQLite database, makes sure all the required
/// tables are initialized and knows how to properly retrieve, create, and
/// store individual entries
pub struct Db {
    /// Connection to SQLite database
    pub connection: Connection,
}

impl Db {
    pub fn init() -> Self {
        let qualifier = "org";
        let organisation = "Open Ape Shop";
        let application = "pinv";

        let dirs = ProjectDirs::from(qualifier, organisation, application).unwrap();

        let data_dir = dirs.data_dir().to_owned();

        let mut db_filepath = data_dir.clone();
        db_filepath.push("pinv.db3");

        if !data_dir.exists() {
            fs::create_dir_all(data_dir.as_path()).unwrap();
        }

        let connection = Connection::open(db_filepath).unwrap();

        // Check to see if the keys table exists in the database...
        
        let query = "SELECT name FROM sqlite_master WHERE type='table' AND name='KEYS'";

        match connection.query_row(query, [], |_| Ok(())).optional().unwrap() {
            Some(_) => {}
            None => {
                let query = "CREATE TABLE KEYS (KEY INTEGER NOT NULL PRIMARY KEY, CATAGORY TEXT NOT NULL)";

                connection.execute(query, []).unwrap();
            }
        }

        Self {
            connection: connection,
        }
    }

    pub fn _new_test() -> Self {
        let connection = Connection::open_in_memory().unwrap();

        // Add a key table to hold all keys we need to store

        let query = "CREATE TABLE KEYS (KEY INTEGER NOT NULL PRIMARY KEY, CATAGORY TEXT NOT NULL)";

        connection.execute(query, []).unwrap();

        Self {
            connection: connection,
        }
    }

    pub fn add_key(&mut self, key: u64, catagory_id: &str) -> Result<(), Box<dyn Error>> {
        let query = format!("INSERT INTO KEYS (KEY, CATAGORY)\nVALUES ({}, '{}')", key, catagory_id);

        self.connection.execute(&query, [])?;
        
        Ok(())
    }

    pub fn add_catagory(&mut self, catagory: Catagory) -> Result<(), Box<dyn Error>> {
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
            query.push_str(format!("{} {}", field.id, field.sql_type()).as_str());

            if i < catagory.fields.len() - 1 {
                query.push(',');
            }
        }

        query.push_str(")");

        self.connection.execute(&query, [])?;

        Ok(())
    }

    pub fn add_entry(&mut self, entry: Entry) -> Result<(), Box<dyn Error>> {
        self.add_key(entry.key, &entry.catagory_id)?;

        let mut query_a = format!(
            "INSERT INTO {} (KEY, LOCATION, QUANTITY, CREATED, MODIFIED",
            entry.catagory_id
        );

        let mut query_b = format!(
            ")\nVALUES ({}, '{}', {}, {}, {}",
            entry.key,
            entry.location,
            entry.quantity,
            entry.created,
            entry.modified
        );

        for field in entry.fields {
            if field.get_sql().len() > 0 {
                query_a.push(',');
                query_b.push(',');
                query_a.push_str(field.id.as_str());
                query_b.push_str(field.get_sql().as_str());
            }
        }

        query_b.push(')');
        query_a.push_str(query_b.as_str());

        let query = query_a;

        match self.connection.execute(&query, []) {
            Ok(_) => Ok(()),
            Err(e) => {
                self.remove_key(entry.key).unwrap();

                Err(Box::new(e))
            }
        }
    }

    /// Get an entry from a query
    ///
    pub fn query_to_entry(&self, query: &str, catagory_id: &str) -> Result<Entry, Box<dyn Error>> {
        let mut statement = self.connection.prepare(query)?;
        let mut column_names = Vec::<String>::new();

        for name in statement.column_names() {
            column_names.push(name.to_string())
        }

        let entry: Entry = statement.query_row([], |row| {
            let mut entry = Entry::new(
                catagory_id,
                row.get(0).unwrap(),
                (row.get::<usize, String>(1).unwrap()).as_str(),
                row.get(2).unwrap(),
                row.get(3).unwrap(),
                row.get(4).unwrap()
            );

            let mut i: usize = 5;
            loop {
                let value: String = match row.get_ref(i) {
                    Ok(result) => format!("{}", Self::sqlval_to_string(result)),
                    Err(e) => match e {
                        SqlError::InvalidColumnIndex(_) => {break;},
                        _ => {
                            return Err(e);
                        }
                    }
                };
                let entry_field = EntryField::new(&column_names[i], &value);
                entry.add_field(entry_field);

                i += 1;
            }

            Ok(entry)
        })?;

        Ok(entry)
    }

    /// Get entries from a query
    ///
    pub fn query_to_entries(&self, query: &str, catagory_id: &str) -> Result<Vec<Entry>, Box<dyn Error>> {
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
                row.get(4)?
            );

            let mut i: usize = 5;
            
            loop {
                let value: String = match row.get_ref(i) {
                    Ok(result) => format!("{}", Self::sqlval_to_string(result)),
                    Err(e) => match e {
                        SqlError::InvalidColumnIndex(_) => {break;},
                        _ => {
                            return Err(Box::new(e));
                        }
                    }
                };
                let entry_field = EntryField::new(&column_names[i], &value);
                entry.add_field(entry_field);

                i += 1;
            }

            entries.push(entry);
        }

        Ok(entries)
    }

    pub fn grab_catagory_fields(&self, name: &str) -> Result<Vec<String>, Box<dyn Error>> {
        let statement = self.connection.prepare(&format!("SELECT * FROM {}", name))?;
        let mut column_names = Vec::<String>::new();

        for name in statement.column_names() {
            column_names.push(name.to_string())
        }

        Ok(column_names)
    }

    pub fn grab_catagory_from_key(&self, key: u64) -> Result<String, Box<dyn Error>> {
        // First we have to figure out which catagory it's in
        let query = format!("SELECT CATAGORY FROM KEYS WHERE KEY={}", key);

        Ok(self.connection.query_row(&query, [], |row| row.get(0))?)
    }

    pub fn grab_entry(&self, key: u64) -> Result<Entry, Box<dyn Error>> {
        // First get the catagory the entry is in
        let catagory = self.grab_catagory_from_key(key)?;

        // Next grab the entry from the catagory
        let query = format!("SELECT * FROM {} WHERE KEY={}", catagory, key);

        self.query_to_entry(&query, &catagory)
    }

    pub fn grab_next_available_key(&self, key: u64) -> Result<u64, Box<dyn Error>> {
        let mut statement = self.connection.prepare("SELECT KEY FROM KEYS WHERE KEY = ?")?;

        let mut key = key;

        loop {
            match statement.query_row(rusqlite::params![key], |_|{Ok(())}).optional()? {
                Some(_) => {},
                None => {break;}
            }

            key += 1
        }

        Ok(key)
    }

    pub fn list_catagories(&self) -> Result<Vec<String>, Box<dyn Error>> {
        // Select all tables excluding the keys table
        let mut statement = self.connection.prepare("SELECT name FROM sqlite_schema WHERE type='table' AND name!='KEYS' ORDER BY name;")?;

        let mut rows = statement.query([])?;

        let mut names = Vec::<String>::new();

        while let Some(row) = rows.next()? {
            names.push(row.get(0)?);
        }
        
        Ok(names)
    }

    pub fn stat_catagories(&self) -> Result<Vec<Vec<String>>, Box<dyn Error>> {
        let catagories = self.list_catagories()?;

        let mut catagory_table = Vec::<Vec<String>>::with_capacity(catagories.len());

        for catagory in catagories {
            let count: usize = self.connection.query_row(&format!("SELECT COUNT(*) FROM {}", catagory), [], |row| row.get(0))?;

            let catagory_row = vec![catagory, count.to_string()];

            catagory_table.push(catagory_row);
        }

        Ok(catagory_table)
    }

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

    pub fn search_catagory(&self, catagory_id: &str, conditions: Vec<&str>) -> Result<Vec<Entry>, Box<dyn Error>> {
        let mut query = format!("SELECT * FROM {} WHERE ", catagory_id);

        for (i, condition) in conditions.iter().enumerate() {
            let condition_split: Vec<&str> = condition.split('=').collect();
            
            if condition_split.len() != 2 {
                bail!("Invalid condition \"{}\"!", condition);
            }

            let id = condition_split[0].to_uppercase();
            let value = condition_split[1];

            query.push_str(format!("{}={}", id, value).as_str());

            query.push_str(match i.cmp(&(conditions.len()-1)){
                cmp::Ordering::Less => " OR ",
                _ => ";",
            })
        }

        self.query_to_entries(&query, catagory_id)
    }

    pub fn fill_svg_template(&self, data: String) -> Result<String, Box<dyn Error>> {
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
    pub fn remove_key(&mut self, key: u64) -> Result<(), Box<dyn Error>> {
        let query = format!("DELETE FROM KEYS WHERE KEY={}", key);

        self.connection.execute(&query, [])?;

        Ok(())
    }

    pub fn mod_entry(&mut self, key: u64, fields: Vec<EntryField>) -> Result<(), Box<dyn Error>> {
        // First get the catagory the entry is in
        let catagory = self.grab_catagory_from_key(key)?;
        let mod_time_string = Local::now().timestamp().to_string();

        let mut fields_str = format!("MODIFIED={},", mod_time_string);

        for (i, field) in fields.iter().enumerate() {
            fields_str.push_str(&format!("{}={}", field.id, field.get_sql()));

            if i < fields.len() - 1 {
                fields_str.push(',')
            }
        }

        // Next grab the entry from the catagory
        let query = format!("UPDATE {} SET {} WHERE KEY={}", catagory, fields_str, key);

        self.connection.execute(&query, [])?;

        Ok(())
    }

    pub fn take(&mut self, key: u64, quantity: u64) -> Result<(), Box<dyn Error>> {
        let entry = self.grab_entry(key)?;

        if entry.quantity < quantity {
            bail!("Tried to take more than the entry had! Has {}, try to take {}!", entry.quantity, quantity);
        }

        let field = EntryField::from_str(&format!("QUANTITY={}", entry.quantity - quantity))?;

        self.mod_entry(key, vec![field])
    }

    pub fn give(&mut self, key: u64, quantity: u64) -> Result<(), Box<dyn Error>> {
        let entry = self.grab_entry(key)?;

        let field = EntryField::from_str(&format!("QUANTITY={}", entry.quantity + quantity))?;

        self.mod_entry(key, vec![field])
    }

    pub fn sqlval_to_string(value: ValueRef) -> String {
        match value {
            ValueRef::Null => "NULL".to_owned(),
            ValueRef::Integer(i) => format!("{}", i),
            ValueRef::Real(f) => format!("{:e}", f),
            ValueRef::Text(s) => format!("'{}'", String::from_utf8_lossy(s)),
            ValueRef::Blob(_) => "BLOB".to_owned(),
        }
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
                    data_type: DataType::TEXT
                },
                CatagoryField {
                    id: "MFCD_BY".to_owned(),
                    data_type: DataType::TEXT
                },
                CatagoryField {
                    id: "OHMS".to_owned(),
                    data_type: DataType::REAL
                },
                CatagoryField {
                    id: "WATTS".to_owned(),
                    data_type: DataType::REAL
                },
                CatagoryField {
                    id: "TOLERANCE".to_owned(),
                    data_type: DataType::REAL
                },
                CatagoryField {
                    id: "PPM_C".to_owned(),
                    data_type: DataType::REAL
                },
                CatagoryField {
                    id: "TERM_STYLE".to_owned(),
                    data_type: DataType::TEXT
                },
                CatagoryField {
                    id: "MAKEUP".to_owned(),
                    data_type: DataType::TEXT
                },
                CatagoryField {
                    id: "CASE_CODE".to_owned(),
                    data_type: DataType::TEXT
                },
                CatagoryField {
                    id: "DATASHEET".to_owned(),
                    data_type: DataType::TEXT
                }
            ]
        }
    }

    // Return test catagory b, 'CAPACITOR'
    pub fn test_catagory_b() -> Catagory {
        Catagory {
            id: "CAPACITOR".to_owned(),
            fields: vec![
                CatagoryField {
                    id: "MPN".to_owned(),
                    data_type: DataType::TEXT,
                },
                CatagoryField {
                    id: "MFCD_BY".to_owned(),
                    data_type: DataType::TEXT
                },
                CatagoryField {
                    id: "FARADS".to_owned(),
                    data_type: DataType::REAL
                },
                CatagoryField {
                    id: "VOLTAGE_DC".to_owned(),
                    data_type: DataType::REAL
                },
                CatagoryField {
                    id: "VOLTAGE_AC".to_owned(),
                    data_type: DataType::REAL
                },
                CatagoryField {
                    id: "HOURS".to_owned(),
                    data_type: DataType::REAL
                },
                CatagoryField {
                    id: "TOLERANCE".to_owned(),
                    data_type: DataType::REAL
                },
                CatagoryField {
                    id: "TERM_STYLE".to_owned(),
                    data_type: DataType::TEXT
                },
                CatagoryField {
                    id: "MAKEUP".to_owned(),
                    data_type: DataType::TEXT
                },
                CatagoryField {
                    id: "CASE_CODE".to_owned(),
                    data_type: DataType::TEXT
                },
                CatagoryField {
                    id: "DATASHEET".to_owned(),
                    data_type: DataType::TEXT
                }
            ]
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
                    value: "'ERJ-PM8F8204V'".to_owned()
                },
                EntryField {
                    id: "MFCD_BY".to_owned(),
                    value: "'Panasonic'".to_owned()
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
                    value: "'SMD'".to_owned()
                },
                EntryField {
                    id: "MAKEUP".to_owned(),
                    value: "'Thick Film'".to_owned()
                },
                EntryField {
                    id: "CASE_CODE".to_owned(),
                    value: "'1206'".to_owned()
                },
                EntryField {
                    id: "DATASHEET".to_owned(),
                    value: "'https://www.mouser.com/datasheet/2/315/Panasonic_Resistor_ERJ_P_PA_PM_Series_022422-2933625.pdf'".to_owned()
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
                EntryField{
                    id: "MPN".to_owned(),
                    value: "'HPCR0819AK39RST'".to_owned()
                },
                EntryField {
                    id: "MFCD_BY".to_owned(),
                    value: "'TE Connectivity/Holsworthy'".to_owned()
                },
                EntryField {
                    id: "OHMS".to_owned(),
                    value: "3.9e1".to_owned()
                },
                EntryField {
                    id: "WATTS".to_owned(),
                    value: "2e0".to_owned()
                },
                EntryField {
                    id: "TOLERANCE".to_owned(),
                    value: "1e1".to_owned()
                },
                EntryField {
                    id: "PPM_C".to_owned(),
                    value: "-8e2".to_owned()
                },
                EntryField {
                    id: "TERM_STYLE".to_owned(),
                    value: "'Through Hole'".to_owned()
                },
                EntryField {
                    id: "MAKEUP".to_owned(),
                    value: "'Ceramic Comp'".to_owned()
                },
                EntryField {
                    id: "CASE_CODE".to_owned(),
                    value: "'19.1x7.9 Axial'".to_owned()
                },
                EntryField {
                    id: "DATASHEET".to_owned(),
                    value: "'https://www.mouser.com/datasheet/2/418/8/ENG_DS_1773193_1_B-2888555.pdf'".to_owned()
                }
            ]
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
                EntryField{
                    id: "MPN".to_owned(),
                    value: "'HPCR0819AK39RST'".to_owned()
                },
                EntryField {
                    id: "MFCD_BY".to_owned(),
                    value: "'Vishay'".to_owned()
                },
                EntryField {
                    id: "FARADS".to_owned(),
                    value: "1.2e-3".to_owned()
                },
                EntryField {
                    id: "VOLTAGE_DC".to_owned(),
                    value: "3.5e1".to_owned()
                },
                EntryField {
                    id: "TOLERANCE".to_owned(),
                    value: "2e-1".to_owned()
                },
                EntryField {
                    id: "TERM_STYLE".to_owned(),
                    value: "'Through Hole'".to_owned()
                },
                EntryField {
                    id: "MAKEUP".to_owned(),
                    value: "'Aluminum Electrolytic'".to_owned()
                },
                EntryField {
                    id: "CASE_CODE".to_owned(),
                    value: "'25x12.5 Radial'".to_owned()
                },
                EntryField {
                    id: "DATASHEET".to_owned(),
                    value: "'https://www.vishay.com/doc?28499'".to_owned()
                }
            ]
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
                EntryField{
                    id: "MPN".to_owned(),
                    value: "'MC08EA220J-TF'".to_owned()
                },
                EntryField {
                    id: "MFCD_BY".to_owned(),
                    value: "'Cornell Dubilier - CDE'".to_owned()
                },
                EntryField {
                    id: "FARADS".to_owned(),
                    value: "2.2e-13".to_owned()
                },
                EntryField {
                    id: "VOLTAGE_DC".to_owned(),
                    value: "1e2".to_owned()
                },
                EntryField {
                    id: "VOLTAGE_AC".to_owned(),
                    value: "7e1".to_owned()
                },
                EntryField {
                    id: "TOLERANCE".to_owned(),
                    value: "5e-2".to_owned()
                },
                EntryField {
                    id: "TERM_STYLE".to_owned(),
                    value: "'SMD'".to_owned()
                },
                EntryField {
                    id: "MAKEUP".to_owned(),
                    value: "'Mica'".to_owned()
                },
                EntryField {
                    id: "CASE_CODE".to_owned(),
                    value: "'0805'".to_owned()
                },
                EntryField {
                    id: "DATASHEET".to_owned(),
                    value: "'https://www.mouser.com/datasheet/2/88/CDUB_S_A0011956908_1-2540249.pdf'".to_owned()
                }
            ]
        }
    }

    // Test creating a field from a string
    #[test]
    fn test_db_create_field_from_string() {
        let field = CatagoryField::from_str("foo:i").unwrap();

        assert_eq!(field.data_type, DataType::INTEGER);
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

        entry.add_field(EntryField::from_str("mpn='ERJ-PM8F8204V'").unwrap());
        entry.add_field(EntryField::from_str("mfcd_by='Panasonic'").unwrap());
        entry.add_field(EntryField::from_str("ohms=8.2e6").unwrap());
        entry.add_field(EntryField::from_str("watts=6.6e-1").unwrap());
        entry.add_field(EntryField::from_str("tolerance=1e-2").unwrap());
        entry.add_field(EntryField::from_str("ppm_c=1e2").unwrap());
        entry.add_field(EntryField::from_str("term_style='SMD'").unwrap());
        entry.add_field(EntryField::from_str("makeup='Thick Film'").unwrap());
        entry.add_field(EntryField::from_str("case_code='1206'").unwrap());
        entry.add_field(EntryField::from_str("datasheet='https://www.mouser.com/datasheet/2/315/Panasonic_Resistor_ERJ_P_PA_PM_Series_022422-2933625.pdf'").unwrap());

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
        
        let test_string: String = format!(r#"ENTRY 0, CATAGORY RESISTOR:
    LOCATION   = 'bazville',
    QUANTITY   = 10,
    CREATED    = {time},
    MODIFIED   = {time},
    MPN        = 'ERJ-PM8F8204V',
    MFCD_BY    = 'Panasonic',
    OHMS       = 8.2e6,
    WATTS      = 6.6e-1,
    TOLERANCE  = 1e-2,
    PPM_C      = 1e2,
    TERM_STYLE = 'SMD',
    MAKEUP     = 'Thick Film',
    CASE_CODE  = '1206',
    DATASHEET  = 'https://www.mouser.com/datasheet/2/315/Panasonic_Resistor_ERJ_P_PA_PM_Series_022422-2933625.pdf'"#,
    time = Local.timestamp(0, 0).to_string());

        assert_eq!(test_string, format!("{}", test_entry_0()));
    }

    #[test]
    fn test_db_search_catagory() {
        let mut db = Db::_new_test();

        db.add_catagory(test_catagory_a()).unwrap();

        db.add_entry(test_entry_0()).unwrap();
        db.add_entry(test_entry_1()).unwrap();

        assert_eq!(db.search_catagory("RESISTOR", vec!["ohms=8.2e6"]).unwrap()[0], test_entry_0());

        assert_eq!(db.search_catagory("RESISTOR", vec!["makeup='Thick Film'", "makeup='Ceramic Comp'"]).unwrap(), vec![test_entry_0(), test_entry_1()]);
    }

    #[test]
    fn test_db_get_catagory_fields() {
        let mut db = Db::_new_test();

        db.add_catagory(test_catagory_a()).unwrap();

        assert_eq!(db.grab_catagory_fields("RESISTOR").unwrap(), vec![
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
        ]);
    }

    #[test]
    fn test_db_modify_entry() {
        let mut db = Db::_new_test();

        db.add_catagory(test_catagory_a()).unwrap();

        db.add_entry(test_entry_0()).unwrap();

        assert_eq!(db.grab_entry(0).unwrap(), test_entry_0());

        db.mod_entry(0, vec![EntryField::from_str("quantity=9").unwrap()]).unwrap();

        assert_eq!(db.grab_entry(0).unwrap().quantity, 9);

        // Test taking and giving
    
        db.give(0, 1).unwrap();

        assert_eq!(db.grab_entry(0).unwrap().quantity, 10);

        db.take(0, 1).unwrap();

        assert_eq!(db.grab_entry(0).unwrap().quantity, 9);
    }
}
