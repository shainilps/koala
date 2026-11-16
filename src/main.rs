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

use serde::{Deserialize, Serialize};
use tiny_http::{Header, Method, Response, Server};

enum Message {
    Get(mpsc::Sender<u32>),
    Inc,
}

struct Registry {
    actors: Mutex<HashMap<String, mpsc::Sender<Message>>>,
}

impl Registry {
    fn new() -> Self {
        Registry {
            actors: Mutex::new(HashMap::new()),
        }
    }

    fn sender_for(self: &Arc<Self>, id: &str) -> mpsc::Sender<Message> {
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

fn main() {
    fs::create_dir_all("data").unwrap();
    let addr = "127.0.0.1:8080";
    let server = Server::http(addr).expect("could not bind");
    println!("listening on http://{addr}");

    let registry = Arc::new(Registry::new());

    for mut request in server.incoming_requests() {
        let reg = Arc::clone(&registry);
        thread::spawn(move || {
            let mut body = String::new();
            let (status, json) = match request.as_reader().read_to_string(&mut body) {
                Ok(_) => handle(reg, request.method(), request.url(), &body),
                Err(_) => (400, error_json("body is not valid utf-8")),
            };

            println!("{} {} -> {status}", request.method(), request.url());

            let content_type = Header::from_bytes("Content-Type", "application/json").unwrap();
            let response = Response::from_string(json)
                .with_status_code(status)
                .with_header(content_type);
            let _ = request.respond(response);
        });
    }
}

fn handle(reg: Arc<Registry>, method: &Method, url: &str, body: &str) -> (u16, String) {
    let Some(id) = actor_id_from_path(url) else {
        return (404, error_json("not found; use POST /actors/{id}/call"));
    };
    if *method != Method::Post {
        return (405, error_json("use POST"));
    }
    let call: Call = match serde_json::from_str(body) {
        Ok(call) => call,
        Err(e) => return (400, error_json(&format!("bad body: {e}"))),
    };

    let actor = reg.sender_for(id);
    match call {
        Call::Inc => {
            if actor.send(Message::Inc).is_err() {
                return (500, error_json("actor is gone"));
            }
            match ask_count(&actor) {
                Some(count) => (200, to_json(&CountReply { count })),
                None => (500, error_json("actor is gone")),
            }
        }
        Call::Get => match ask_count(&actor) {
            Some(count) => (200, to_json(&CountReply { count })),
            None => (500, error_json("actor is gone")),
        },
    }
}

#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "lowercase")]
enum Call {
    Inc,
    Get,
}

#[derive(Serialize)]
struct CountReply {
    count: u32,
}

#[derive(Serialize)]
struct ErrorReply<'a> {
    error: &'a str,
}

fn to_json<T: Serialize>(value: &T) -> String {
    serde_json::to_string(value).expect("reply types always serialize")
}

fn error_json(msg: &str) -> String {
    to_json(&ErrorReply { error: msg })
}

fn actor_id_from_path(url: &str) -> Option<&str> {
    let id = url.strip_prefix("/actors/")?.strip_suffix("/call")?;
    if id.is_empty() || id.contains('/') {
        return None;
    }
    Some(id)
}

fn ask_count(actor: &mpsc::Sender<Message>) -> Option<u32> {
    let (reply_tx, reply_rx) = mpsc::channel();
    actor.send(Message::Get(reply_tx)).ok()?;
    reply_rx.recv().ok()
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
