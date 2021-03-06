use super::blake2::blake2b::blake2b;
use super::COLUMNS;
use super::{ClientDB, DBError, DBValue};
use std::collections::{HashMap, HashSet};
use std::sync::RwLock;

type DBHashMap = HashMap<Vec<u8>, Vec<u8>>;
type ColumnHashSet = HashSet<String>;

/// An in-memory database implementing the ClientDB trait.
///
/// It is not particularily optimized, it exists for ease and speed of testing. It's not expected
/// this DB would be used outside of tests.
pub struct MemoryDB {
    db: RwLock<DBHashMap>,
    known_columns: RwLock<ColumnHashSet>,
}

impl MemoryDB {
    /// Open the in-memory database.
    ///
    /// All columns must be supplied initially, you will get an error if you try to access a column
    /// that was not declared here. This condition is enforced artificially to simulate RocksDB.
    pub fn open() -> Self {
        let db: DBHashMap = HashMap::new();
        let mut known_columns: ColumnHashSet = HashSet::new();
        for col in &COLUMNS {
            known_columns.insert(col.to_string());
        }
        Self {
            db: RwLock::new(db),
            known_columns: RwLock::new(known_columns),
        }
    }

    /// Hashes a key and a column name in order to get a unique key for the supplied column.
    fn get_key_for_col(col: &str, key: &[u8]) -> Vec<u8> {
        blake2b(32, col.as_bytes(), key).as_bytes().to_vec()
    }
}

impl ClientDB for MemoryDB {
    /// Get the value of some key from the database. Returns `None` if the key does not exist.
    fn get(&self, col: &str, key: &[u8]) -> Result<Option<DBValue>, DBError> {
        // Panic if the DB locks are poisoned.
        let db = self.db.read().unwrap();
        let known_columns = self.known_columns.read().unwrap();

        if known_columns.contains(&col.to_string()) {
            let column_key = MemoryDB::get_key_for_col(col, key);
            Ok(db.get(&column_key).and_then(|val| Some(val.clone())))
        } else {
            Err(DBError {
                message: "Unknown column".to_string(),
            })
        }
    }

    /// Puts a key in the database.
    fn put(&self, col: &str, key: &[u8], val: &[u8]) -> Result<(), DBError> {
        // Panic if the DB locks are poisoned.
        let mut db = self.db.write().unwrap();
        let known_columns = self.known_columns.read().unwrap();

        if known_columns.contains(&col.to_string()) {
            let column_key = MemoryDB::get_key_for_col(col, key);
            db.insert(column_key, val.to_vec());
            Ok(())
        } else {
            Err(DBError {
                message: "Unknown column".to_string(),
            })
        }
    }

    /// Return true if some key exists in some column.
    fn exists(&self, col: &str, key: &[u8]) -> Result<bool, DBError> {
        // Panic if the DB locks are poisoned.
        let db = self.db.read().unwrap();
        let known_columns = self.known_columns.read().unwrap();

        if known_columns.contains(&col.to_string()) {
            let column_key = MemoryDB::get_key_for_col(col, key);
            Ok(db.contains_key(&column_key))
        } else {
            Err(DBError {
                message: "Unknown column".to_string(),
            })
        }
    }

    /// Delete some key from the database.
    fn delete(&self, col: &str, key: &[u8]) -> Result<(), DBError> {
        // Panic if the DB locks are poisoned.
        let mut db = self.db.write().unwrap();
        let known_columns = self.known_columns.read().unwrap();

        if known_columns.contains(&col.to_string()) {
            let column_key = MemoryDB::get_key_for_col(col, key);
            db.remove(&column_key);
            Ok(())
        } else {
            Err(DBError {
                message: "Unknown column".to_string(),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::stores::{BLOCKS_DB_COLUMN, VALIDATOR_DB_COLUMN};
    use super::super::ClientDB;
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn test_memorydb_can_delete() {
        let col_a: &str = BLOCKS_DB_COLUMN;

        let db = MemoryDB::open();

        db.put(col_a, "dogs".as_bytes(), "lol".as_bytes()).unwrap();

        assert_eq!(
            db.get(col_a, "dogs".as_bytes()).unwrap().unwrap(),
            "lol".as_bytes()
        );

        db.delete(col_a, "dogs".as_bytes()).unwrap();

        assert_eq!(db.get(col_a, "dogs".as_bytes()).unwrap(), None);
    }

    #[test]
    fn test_memorydb_column_access() {
        let col_a: &str = BLOCKS_DB_COLUMN;
        let col_b: &str = VALIDATOR_DB_COLUMN;

        let db = MemoryDB::open();

        /*
         * Testing that if we write to the same key in different columns that
         * there is not an overlap.
         */
        db.put(col_a, "same".as_bytes(), "cat".as_bytes()).unwrap();
        db.put(col_b, "same".as_bytes(), "dog".as_bytes()).unwrap();

        assert_eq!(
            db.get(col_a, "same".as_bytes()).unwrap().unwrap(),
            "cat".as_bytes()
        );
        assert_eq!(
            db.get(col_b, "same".as_bytes()).unwrap().unwrap(),
            "dog".as_bytes()
        );
    }

    #[test]
    fn test_memorydb_unknown_column_access() {
        let col_a: &str = BLOCKS_DB_COLUMN;
        let col_x: &str = "ColumnX";

        let db = MemoryDB::open();

        /*
         * Test that we get errors when using undeclared columns
         */
        assert!(db.put(col_a, "cats".as_bytes(), "lol".as_bytes()).is_ok());
        assert!(db.put(col_x, "cats".as_bytes(), "lol".as_bytes()).is_err());

        assert!(db.get(col_a, "cats".as_bytes()).is_ok());
        assert!(db.get(col_x, "cats".as_bytes()).is_err());
    }

    #[test]
    fn test_memorydb_exists() {
        let col_a: &str = BLOCKS_DB_COLUMN;
        let col_b: &str = VALIDATOR_DB_COLUMN;

        let db = MemoryDB::open();

        /*
         * Testing that if we write to the same key in different columns that
         * there is not an overlap.
         */
        db.put(col_a, "cats".as_bytes(), "lol".as_bytes()).unwrap();

        assert_eq!(true, db.exists(col_a, "cats".as_bytes()).unwrap());
        assert_eq!(false, db.exists(col_b, "cats".as_bytes()).unwrap());

        assert_eq!(false, db.exists(col_a, "dogs".as_bytes()).unwrap());
        assert_eq!(false, db.exists(col_b, "dogs".as_bytes()).unwrap());
    }

    #[test]
    fn test_memorydb_threading() {
        let col_name: &str = BLOCKS_DB_COLUMN;

        let db = Arc::new(MemoryDB::open());

        let thread_count = 10;
        let write_count = 10;

        // We're execting the product of these numbers to fit in one byte.
        assert!(thread_count * write_count <= 255);

        let mut handles = vec![];
        for t in 0..thread_count {
            let wc = write_count;
            let db = db.clone();
            let col = col_name.clone();
            let handle = thread::spawn(move || {
                for w in 0..wc {
                    let key = (t * w) as u8;
                    let val = 42;
                    db.put(&col, &vec![key], &vec![val]).unwrap();
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        for t in 0..thread_count {
            for w in 0..write_count {
                let key = (t * w) as u8;
                let val = db.get(&col_name, &vec![key]).unwrap().unwrap();
                assert_eq!(vec![42], val);
            }
        }
    }
}
