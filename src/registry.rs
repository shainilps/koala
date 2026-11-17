use std::{
    collections::HashMap,
    fs::{self, File},
    io::{self, Write},
    sync::{
        Arc, Mutex,
        mpsc::{self, RecvTimeoutError},
    },
    thread,
    time::Duration,
};

pub struct Registry {
    actors: Mutex<HashMap<String, mpsc::Sender<Message>>>,
}

pub enum Message {
    Get(mpsc::Sender<u32>),
    Inc,
}

impl Registry {
    pub fn new() -> Self {
        Registry {
            actors: Mutex::new(HashMap::new()),
        }
    }

    pub fn sender_for(self: &Arc<Self>, id: &str) -> mpsc::Sender<Message> {
        let mut actors = self.actors.lock().unwrap();
        if let Some(tx) = actors.get(id) {
            return tx.clone();
        } else {
            let tx = create_actor(id.to_string(), Arc::clone(self));
            actors.insert(id.to_string(), tx.clone());
            return tx;
        }
    }
}

fn create_actor(id: String, reg: Arc<Registry>) -> mpsc::Sender<Message> {
    let (tx, rx) = mpsc::channel::<Message>();
    println!("starting {id}");
    thread::spawn(move || {
        let mut count: u32 = read_file(&id).unwrap();
        loop {
            let msg = match rx.recv_timeout(Duration::from_secs(10)) {
                Ok(msg) => msg,
                Err(RecvTimeoutError::Disconnected) => break,
                Err(RecvTimeoutError::Timeout) => {
                    let mut actors = reg.actors.lock().unwrap();
                    match rx.try_recv() {
                        Ok(msg) => msg,
                        Err(_) => {
                            actors.remove(&id);
                            println!("sleeping {id}");
                            break;
                        }
                    }
                }
            };

            match msg {
                Message::Inc => {
                    count = count + 1;
                    write_file(&id, count).unwrap();
                }
                Message::Get(reply) => {
                    reply.send(count).unwrap();
                }
            }
        }
    });
    tx
}

fn write_file(actor_id: &str, data: u32) -> Result<(), io::Error> {
    let path = format!("data/{actor_id}.json");
    let tmp = format!("data/{actor_id}.json.tmp");

    let text = serde_json::to_string(&data)?;
    let mut file = File::create(&tmp)?;
    file.write_all(text.as_bytes())?;
    file.sync_all()?;
    fs::rename(&tmp, &path)?;
    Ok(())
}

fn read_file(actor_id: &str) -> Result<u32, io::Error> {
    let path = format!("data/{actor_id}.json");

    match fs::read_to_string(&path) {
        Ok(text) => {
            let count: u32 = serde_json::from_str(&text)?;
            Ok(count)
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(0),
        Err(e) => Err(e),
    }
}
