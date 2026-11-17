use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;

pub trait ActorType {
    type State: Serialize + DeserializeOwned + Default;
    fn handle(state: &mut Self::State, action: &str, args: Value) -> Result<Value, String>;
}
