use rusqlite::{Connection, OptionalExtension};
use simple_error::bail;
use std::{error::Error, fmt::format};

/// Mapping to SQLite datatypes
#[derive(Debug, Clone, PartialEq)]
pub enum DataType {
    NULL,
    INTEGER,
    REAL,
    TEXT,
    BLOB,
}

/// Used to define fields in catagories
#[derive(Debug, Clone)]
pub struct Field {
    pub id: String,
    pub data_type: DataType,
}

impl Field {
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
            id: split_str[0].to_owned(),
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
}

/// Used to help define catagories(which are translated directly into sql tables)
#[derive(Debug, Clone)]
pub struct Catagory {
    pub id: String,
    pub fields: Vec<Field>,
}

impl Catagory {
    pub fn new(id: &str) -> Self {
        Self {
            id: id.to_owned(),
            fields: Vec::new(),
        }
    }

    pub fn add_field(&mut self, field: Field) {
        self.fields.push(field);
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
    pub fn _new_test() -> Self {
        let connection = Connection::open_in_memory().unwrap();

        Self {
            connection: connection,
        }
    }

    pub fn add_catagory(&mut self, catagory: Catagory) -> Result<(), Box<dyn Error>> {
        // Check to see if the table exists first...
        let query = format!(
            r#"SELECT name FROM sqlite_master WHERE type='table' AND name='{}';"#,
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

            _ => {
            }
        }

        // Otherwise, add the catagory to the database
        let mut query = format!("CREATE TABLE {} (\n", catagory.id);

        for (i, field) in catagory.fields.iter().enumerate() {
            query.push_str(format!("\t{} {}", field.id, field.sql_type()).as_str());

            if i < catagory.fields.len() - 1 {
                query.push(',');
            }

            query.push('\n');
        }

        query.push_str(");");

        eprintln!("{}", query);

        self.connection.execute(&query, [])?;

        Ok(())
    }

    pub fn add_entry(&mut self) -> Result<(), Box<dyn Error>> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Test creating a field from a string
    #[test]
    fn test_db_create_field_from_string() {
        let field = Field::from_str("foo:i").unwrap();

        assert_eq!(field.data_type, DataType::INTEGER);
    }

    // Test getting the sql type from a field
    #[test]
    fn test_db_get_sql_type() {
        let field = Field::from_str("foo:i").unwrap();
        
        assert_eq!(field.sql_type(), "INTEGER");
    }
    // Test adding a catagory to a database
    #[test]
    fn test_db_add_catagory() {
        let mut db = Db::_new_test();
        let mut catagory = Catagory::new("foo");

        catagory.add_field(Field::from_str("baz:i").unwrap());
        catagory.add_field(Field::from_str("bar:t").unwrap());

        // Shouldn't fail the first time
        db.add_catagory(catagory.clone()).unwrap();

        // Should fail the second time
        db.add_catagory(catagory).unwrap_err();
    }
}
