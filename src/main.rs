use std::{
    fs::{self},
    sync::{
        Arc,
        mpsc::{self},
    },
    thread,
};

use serde::{Deserialize, Serialize};
use tiny_http::{Header, Method, Response, Server};

mod registry;
mod storage;
mod actor;

fn main() {
    fs::create_dir_all("data").unwrap();
    let addr = "127.0.0.1:8080";
    let server = Server::http(addr).expect("could not bind");
    println!("listening on http://{addr}");

    let registry = Arc::new(registry::Registry::new());

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

fn handle(reg: Arc<registry::Registry>, method: &Method, url: &str, body: &str) -> (u16, String) {
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
            if actor.send(registry::Message::Inc).is_err() {
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

fn ask_count(actor: &mpsc::Sender<registry::Message>) -> Option<u32> {
    let (reply_tx, reply_rx) = mpsc::channel();
    actor.send(registry::Message::Get(reply_tx)).ok()?;
    reply_rx.recv().ok()
}
