# koala

A tiny actor runtime in Rust, built to learn how Rivet-style actors work.

Each actor is a named piece of state with its own thread and mailbox, so messages are handled one at a time
with no locks. The first message to a name creates or wakes the actor, it saves its own state to disk, and it
sleeps after 10 seconds idle.

## API

```
POST http://127.0.0.1:8080/actors/{type}:{name}/call
{"action": "<action>", "args": {...}}
```

`{type}` is the kind of actor, `{name}` is anything you choose. Replies are JSON; errors are `{"error": "..."}`.

```
curl -s -X POST http://127.0.0.1:8080/actors/counter:visits/call -d '{"action":"inc"}'
{"count":1}
```

## Actors

### counter

A number that only goes up.

- `inc`: add one, returns `{"count"}`
- `get`: returns `{"count"}`

### kb

A vector store: chunks of text with a vector and an optional tag, searched by cosine similarity.

- `put`: `text`, `vector`, `tag?` stores a chunk
- `search`: `vector`, `limit?` (3), `tag?`, `with_vector?` returns `{"results": [{"score", "text", "tag"}]}`, best first
- `put_text`: `text`, `tag?` like `put`, the vector comes from a local embedding model
- `search_text`: `query`, `limit?`, `tag?`, `with_vector?` like `search`, the vector comes from the model
- `get`: lists every chunk

`put_text` and `search_text` need the server started with `EMBED_URL` and `EMBED_MODEL` (a local Ollama model).

## Example

```
EMBED_URL=http://127.0.0.1:11434 EMBED_MODEL=all-minilm cargo run
```
