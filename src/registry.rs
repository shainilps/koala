use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        mpsc::{self, RecvTimeoutError},
    },
    thread,
    time::Duration,
};

use serde_json::Value;

use crate::{
    actor::{Actor, Counter},
    storage::{self, StorageKind},
};

pub struct Registry {
    actors: Mutex<HashMap<String, mpsc::Sender<Message>>>,
    storage: StorageKind,
}

pub struct Message {
    pub action: String,
    pub args: Value,
    pub reply: mpsc::Sender<Result<Value, String>>,
}

impl Registry {
    pub fn new(storage: StorageKind) -> Self {
        Registry {
            actors: Mutex::new(HashMap::new()),
            storage,
        }
    }

    pub fn sender_for(self: &Arc<Self>, id: &str) -> Result<mpsc::Sender<Message>, String> {
        let mut actors = self.actors.lock().unwrap();
        if let Some(tx) = actors.get(id) {
            return Ok(tx.clone());
        }
        let tx = match id.split_once(':') {
            Some(("counter", name)) if !name.is_empty() => {
                create_actor(id.to_string(), Arc::clone(self), Counter::default())
            }
            _ => return Err(format!("unknown actor type in id: {id}")),
        };
        actors.insert(id.to_string(), tx.clone());
        Ok(tx)
    }
}

fn create_actor<A: Actor + Send + 'static>(
    id: String,
    reg: Arc<Registry>,
    actor: A,
) -> mpsc::Sender<Message> {
    let (tx, rx) = mpsc::channel::<Message>();
    println!("starting {id}");
    thread::spawn(move || {
        if let Err(e) = run_actor(&id, &reg, rx, actor) {
            println!("{id} stopped: {e}");
            reg.actors.lock().unwrap().remove(&id);
        }
    });
    tx
}

fn run_actor<A: Actor>(
    id: &str,
    reg: &Registry,
    rx: mpsc::Receiver<Message>,
    mut actor: A,
) -> Result<(), String> {
    let storage = storage::open(reg.storage, id).map_err(|e| format!("open storage: {e}"))?;

    actor.wake(&*storage)?;

    loop {
        let msg = match rx.recv_timeout(Duration::from_secs(10)) {
            Ok(msg) => msg,
            Err(RecvTimeoutError::Disconnected) => return Ok(()),
            Err(RecvTimeoutError::Timeout) => {
                let mut actors = reg.actors.lock().unwrap();
                match rx.try_recv() {
                    Ok(msg) => msg,
                    Err(_) => {
                        actors.remove(id);
                        println!("sleeping {id}");
                        return Ok(());
                    }
                }
            }
        };

        match actor.handle(&*storage, &msg.action, msg.args) {
            Ok(value) => {
                let _ = msg.reply.send(Ok(value));
            }
            Err(e) => {
                let _ = msg.reply.send(Err(e));
            }
        }
    }
}
