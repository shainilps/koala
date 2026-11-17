use std::{
    fs::{self, File},
    io::{self, Write},
    time::Duration,
};

use rusqlite::{Connection, OptionalExtension};

pub trait Storage {
    fn load(&self) -> io::Result<Option<Vec<u8>>>;
    fn save(&self, bytes: &[u8]) -> io::Result<()>;
}

#[derive(Clone, Copy)]
pub enum StorageKind {
    File,
    Sqlite,
}

pub fn open(kind: StorageKind, id: &str) -> io::Result<Box<dyn Storage>> {
    match kind {
        StorageKind::File => Ok(Box::new(FileStorage::new(id)?)),
        StorageKind::Sqlite => Ok(Box::new(SqliteStorage::new(id)?)),
    }
}

pub struct FileStorage {
    path: String,
    tmp: String,
}

impl FileStorage {
    pub fn new(id: &str) -> io::Result<Self> {
        fs::create_dir_all("data")?;
        Ok(FileStorage {
            path: format!("data/{id}.json"),
            tmp: format!("data/{id}.json.tmp"),
        })
    }
}

impl Storage for FileStorage {
    fn load(&self) -> io::Result<Option<Vec<u8>>> {
        match fs::read(&self.path) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e),
        }
    }

    fn save(&self, bytes: &[u8]) -> io::Result<()> {
        let mut file = File::create(&self.tmp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        fs::rename(&self.tmp, &self.path)?;
        Ok(())
    }
}

pub struct SqliteStorage {
    conn: Connection,
    id: String,
}

impl SqliteStorage {
    pub fn new(id: &str) -> io::Result<Self> {
        fs::create_dir_all("data")?;
        let conn = Connection::open("data/actors.db").map_err(io::Error::other)?;
        conn.busy_timeout(Duration::from_secs(5))
            .map_err(io::Error::other)?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS actors (id TEXT PRIMARY KEY, state BLOB NOT NULL)",
            (),
        )
        .map_err(io::Error::other)?;
        Ok(SqliteStorage {
            conn,
            id: id.to_string(),
        })
    }
}

impl Storage for SqliteStorage {
    fn load(&self) -> io::Result<Option<Vec<u8>>> {
        self.conn
            .query_row(
                "SELECT state FROM actors WHERE id = ?1",
                (&self.id,),
                |row| row.get(0),
            )
            .optional()
            .map_err(io::Error::other)
    }

    fn save(&self, bytes: &[u8]) -> io::Result<()> {
        self.conn
            .execute(
                "INSERT INTO actors (id, state) VALUES (?1, ?2)
                 ON CONFLICT(id) DO UPDATE SET state = excluded.state",
                (&self.id, bytes),
            )
            .map_err(io::Error::other)?;
        Ok(())
    }
}
