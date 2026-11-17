use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::storage::Storage;

pub trait Actor {
    fn wake(&mut self, store: &dyn Storage) -> Result<(), String>;
    fn handle(
        &mut self,
        store: &dyn Storage,
        action: &str,
        args: Value,
    ) -> Result<Value, String>;
}

#[derive(Serialize, Deserialize, Default)]
pub struct Counter {
    count: u32,
}

impl Actor for Counter {
    fn wake(&mut self, store: &dyn Storage) -> Result<(), String> {
        let saved = store.load().map_err(|e| format!("load: {e}"))?;
        if let Some(bytes) = saved {
            *self = serde_json::from_slice(&bytes).map_err(|e| format!("bad state: {e}"))?;
        }
        Ok(())
    }

    fn handle(
        &mut self,
        store: &dyn Storage,
        action: &str,
        _args: Value,
    ) -> Result<Value, String> {
        match action {
            "inc" => {
                self.count += 1;
                let bytes = serde_json::to_vec(self).map_err(|e| e.to_string())?;
                if let Err(e) = store.save(&bytes) {
                    self.count -= 1;
                    return Err(format!("save failed: {e}"));
                }
                Ok(json!({ "count": self.count }))
            }
            "get" => Ok(json!({ "count": self.count })),
            _ => Err(format!("unknown action: {action}")),
        }
    }
}
