use std::{
    sync::{
        Arc,
        mpsc::{self},
    },
    thread,
};

use serde::Deserialize;
use serde_json::{Value, json};
use tiny_http::{Header, Method, Response, Server};

mod actor;
mod registry;
mod storage;

fn main() {
    let addr = "127.0.0.1:8080";
    let server = Server::http(addr).expect("could not bind");
    println!("listening on http://{addr}");

    let registry = Arc::new(registry::Registry::new(storage::StorageKind::File));

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

    let actor = match reg.sender_for(id) {
        Ok(actor) => actor,
        Err(e) => return (404, error_json(&e)),
    };

    let (reply_tx, reply_rx) = mpsc::channel();
    let msg = registry::Message {
        action: call.action,
        args: call.args,
        reply: reply_tx,
    };
    if actor.send(msg).is_err() {
        return (500, error_json("actor is gone"));
    }
    match reply_rx.recv() {
        Ok(Ok(value)) => (200, value.to_string()),
        Ok(Err(e)) => (400, error_json(&e)),
        Err(_) => (500, error_json("actor is gone")),
    }
}

#[derive(Deserialize)]
struct Call {
    action: String,
    #[serde(default)]
    args: Value,
}

fn error_json(msg: &str) -> String {
    json!({ "error": msg }).to_string()
}

fn actor_id_from_path(url: &str) -> Option<&str> {
    let id = url.strip_prefix("/actors/")?.strip_suffix("/call")?;
    if id.is_empty() || id.contains('/') {
        return None;
    }
    Some(id)
}
