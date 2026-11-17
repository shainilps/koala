use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Value, json};

pub trait Actor: Serialize + DeserializeOwned {
    fn handle(&mut self, action: &str, args: Value) -> Result<Value, String>;
}

#[derive(Serialize, Deserialize, Default)]
pub struct Counter {
    count: u32,
}

impl Actor for Counter {
    fn handle(&mut self, action: &str, _args: Value) -> Result<Value, String> {
        match action {
            "inc" => {
                self.count += 1;
                Ok(json!({ "count": self.count }))
            }
            "get" => Ok(json!({ "count": self.count })),
            _ => Err(format!("unknown action: {action}")),
        }
    }
}
