use std::{
    env,
    io::{Read, Write},
    net::TcpStream,
    time::Duration,
};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::storage::Storage;

pub trait Actor {
    fn wake(&mut self, store: &dyn Storage) -> Result<(), String>;
    fn handle(&mut self, store: &dyn Storage, action: &str, args: Value) -> Result<Value, String>;
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

    fn handle(&mut self, store: &dyn Storage, action: &str, _args: Value) -> Result<Value, String> {
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

#[derive(Serialize, Deserialize)]
struct Chunk {
    text: String,
    tag: String,
    vector: Vec<f32>,
}

#[derive(Default)]
pub struct KBActor {
    chunks: Vec<Chunk>,
}

impl KBActor {
    fn put_chunk(
        &mut self,
        store: &dyn Storage,
        text: &str,
        tag: &str,
        vector: Vec<f32>,
    ) -> Result<Value, String> {
        if vector.is_empty() {
            return Err("vector is empty".to_string());
        }
        if vector.iter().all(|x| *x == 0.0) {
            return Err("vector is all zeros".to_string());
        }
        if let Some(first) = self.chunks.first() {
            if first.vector.len() != vector.len() {
                return Err(format!(
                    "vector has {} numbers, this kb uses {}",
                    vector.len(),
                    first.vector.len()
                ));
            }
        }

        let chunk = Chunk {
            text: text.to_string(),
            tag: tag.to_string(),
            vector,
        };
        let bytes = postcard::to_allocvec(&chunk).map_err(|e| e.to_string())?;
        store
            .append(&bytes)
            .map_err(|e| format!("save failed: {e}"))?;
        self.chunks.push(chunk);
        Ok(json!({ "count": self.chunks.len() }))
    }

    fn search_chunks(&self, query: &[f32], args: &Value) -> Result<Value, String> {
        let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(3) as usize;
        let tag = args.get("tag").and_then(|v| v.as_str());
        let with_vector = args
            .get("with_vector")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let Some(first) = self.chunks.first() else {
            return Ok(json!({ "results": [] }));
        };
        if first.vector.len() != query.len() {
            return Err(format!(
                "vector has {} numbers, this kb uses {}",
                query.len(),
                first.vector.len()
            ));
        }
        let query_len = query.iter().map(|x| x * x).sum::<f32>().sqrt();
        if query_len == 0.0 {
            return Err("vector is all zeros".to_string());
        }

        let mut hits: Vec<(f32, usize)> = Vec::new();
        for (id, chunk) in self.chunks.iter().enumerate() {
            if let Some(tag) = tag {
                if chunk.tag != tag {
                    continue;
                }
            }
            let mut dot = 0.0;
            let mut chunk_sq = 0.0;
            for i in 0..query.len() {
                dot += query[i] * chunk.vector[i];
                chunk_sq += chunk.vector[i] * chunk.vector[i];
            }
            let distance = 1.0 - dot / (query_len * chunk_sq.sqrt());
            hits.push((distance, id));
        }
        hits.sort_by(|a, b| a.0.total_cmp(&b.0));
        hits.truncate(limit);

        let results: Vec<Value> = hits
            .iter()
            .map(|&(distance, id)| {
                let mut item = json!({
                    "score": 1.0 - distance,
                    "text": self.chunks[id].text,
                    "tag": self.chunks[id].tag,
                });
                if with_vector {
                    item["vector"] = json!(self.chunks[id].vector);
                }
                item
            })
            .collect();
        Ok(json!({ "results": results }))
    }
}

fn embed(text: &str) -> Result<Vec<f32>, String> {
    let Ok(url) = env::var("EMBED_URL") else {
        return Err("text embedding is not available: EMBED_URL is not set".to_string());
    };
    let Ok(model) = env::var("EMBED_MODEL") else {
        return Err("text embedding is not available: EMBED_MODEL is not set".to_string());
    };
    let Some(rest) = url.strip_prefix("http://") else {
        return Err(format!(
            "text embedding is not available: only a local http model is supported, got {url}"
        ));
    };
    let host = rest.split('/').next().unwrap_or("");
    let addr = if host.contains(':') {
        host.to_string()
    } else {
        format!("{host}:80")
    };

    let mut stream = TcpStream::connect(&addr)
        .map_err(|e| format!("cannot reach embedding server at {url}: {e}"))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(30)))
        .map_err(|e| e.to_string())?;

    let body = json!({ "model": model, "input": text }).to_string();
    let request = format!(
        "POST /api/embed HTTP/1.0\r\nHost: {host}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    );
    stream
        .write_all(request.as_bytes())
        .map_err(|e| format!("embedding request failed: {e}"))?;

    let mut response = Vec::new();
    stream
        .read_to_end(&mut response)
        .map_err(|e| format!("embedding response failed: {e}"))?;

    let Some(split) = response.windows(4).position(|w| w == b"\r\n\r\n") else {
        return Err("embedding server sent a bad response".to_string());
    };
    let head = String::from_utf8_lossy(&response[..split]);
    let status = head
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .unwrap_or("");
    let reply: Value = serde_json::from_slice(&response[split + 4..])
        .map_err(|e| format!("embedding server sent bad json: {e}"))?;

    if status != "200" {
        let msg = reply
            .get("error")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown error");
        return Err(format!("embedding server error {status}: {msg}"));
    }

    let Some(numbers) = reply
        .get("embeddings")
        .and_then(|v| v.get(0))
        .and_then(|v| v.as_array())
    else {
        return Err("embedding server sent no embeddings".to_string());
    };
    let mut vector: Vec<f32> = Vec::with_capacity(numbers.len());
    for n in numbers {
        let Some(x) = n.as_f64() else {
            return Err("embedding server sent a non-number".to_string());
        };
        vector.push(x as f32);
    }
    Ok(vector)
}

impl Actor for KBActor {
    fn wake(&mut self, store: &dyn Storage) -> Result<(), String> {
        let Some(bytes) = store.load().map_err(|e| format!("load: {e}"))? else {
            return Ok(());
        };
        let mut rest: &[u8] = &bytes;
        while !rest.is_empty() {
            let (chunk, tail) =
                postcard::take_from_bytes::<Chunk>(rest).map_err(|e| format!("bad chunk: {e}"))?;
            self.chunks.push(chunk);
            rest = tail;
        }
        Ok(())
    }

    fn handle(&mut self, store: &dyn Storage, action: &str, args: Value) -> Result<Value, String> {
        match action {
            "put" => {
                let Some(text) = args.get("text").and_then(|v| v.as_str()) else {
                    return Err("missing text".to_string());
                };
                let tag = args.get("tag").and_then(|v| v.as_str()).unwrap_or("");
                let Some(numbers) = args.get("vector").and_then(|v| v.as_array()) else {
                    return Err("missing vector".to_string());
                };
                let mut vector: Vec<f32> = Vec::with_capacity(numbers.len());
                for n in numbers {
                    let Some(x) = n.as_f64() else {
                        return Err("vector must be an array of numbers".to_string());
                    };
                    vector.push(x as f32);
                }
                self.put_chunk(store, text, tag, vector)
            }
            "put_text" => {
                let Some(text) = args.get("text").and_then(|v| v.as_str()) else {
                    return Err("missing text".to_string());
                };
                let tag = args.get("tag").and_then(|v| v.as_str()).unwrap_or("");
                let vector = embed(text)?;
                self.put_chunk(store, text, tag, vector)
            }
            "search" => {
                let Some(numbers) = args.get("vector").and_then(|v| v.as_array()) else {
                    return Err("missing vector".to_string());
                };
                let mut query: Vec<f32> = Vec::with_capacity(numbers.len());
                for n in numbers {
                    let Some(x) = n.as_f64() else {
                        return Err("vector must be an array of numbers".to_string());
                    };
                    query.push(x as f32);
                }
                self.search_chunks(&query, &args)
            }
            "search_text" => {
                let Some(text) = args.get("query").and_then(|v| v.as_str()) else {
                    return Err("missing query".to_string());
                };
                let query = embed(text)?;
                self.search_chunks(&query, &args)
            }
            "get" => {
                let chunks: Vec<Value> = self
                    .chunks
                    .iter()
                    .enumerate()
                    .map(|(id, chunk)| json!({ "id": id, "text": chunk.text, "tag": chunk.tag }))
                    .collect();
                let dim = self.chunks.first().map(|c| c.vector.len()).unwrap_or(0);
                Ok(json!({ "count": self.chunks.len(), "dim": dim, "chunks": chunks }))
            }
            _ => Err(format!("unknown action: {action}")),
        }
    }
}
