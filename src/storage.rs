use std::io;

pub trait Storage {
    fn load(&self) -> io::Result<Option<Vec<u8>>>;
    fn save(&self, bytes: &[u8]) -> io::Result<()>;
}
