use std::{
    fs::{self, File},
    io::{self, Write},
};

pub trait Storage {
    fn load(&self) -> io::Result<Option<Vec<u8>>>;
    fn save(&self, bytes: &[u8]) -> io::Result<()>;
}

#[derive(Clone, Copy)]
pub enum StorageKind {
    File,
}

pub fn open(kind: StorageKind, id: &str) -> io::Result<Box<dyn Storage>> {
    match kind {
        StorageKind::File => Ok(Box::new(FileStorage::new(id)?)),
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
