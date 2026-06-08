# Skill: OpenAI API

**Trigger:** openai, GPT, ChatGPT API, completion, chat

**Description:** Intégration OpenAI API : chat completions, streaming, function calling, embeddings, gestion d'erreurs, rate limiting.

## Body

### Chat Completion (Rust)
```rust
let client = reqwest::Client::new();
let resp = client
    .post("https://api.openai.com/v1/chat/completions")
    .header("Authorization", format!("Bearer {}", api_key))
    .json(&json!({
        "model": "gpt-4o",
        "messages": [
            {"role": "system", "content": "You are a helpful assistant."},
            {"role": "user", "content": "Explain Rust ownership"}
        ],
        "temperature": 0.7,
        "max_tokens": 500
    }))
    .send().await?;
```

### Streaming
```rust
let mut stream = client
    .post("...")
    .json(&json!({"stream": true, ...}))
    .send().await?
    .bytes_stream();

while let Some(chunk) = stream.next().await {
    let text = parse_sse_chunk(&chunk?)?;
    print!("{text}");
}
```

### Function Calling
```json
{
    "tools": [{
        "type": "function",
        "function": {
            "name": "get_weather",
            "description": "Get current weather",
            "parameters": {
                "type": "object",
                "properties": {
                    "city": {"type": "string"}
                }
            }
        }
    }]
}
```

### Rate Limiting & Retry
```rust
// Exponential backoff avec reqwest-retry ou manuel
let mut delay = Duration::from_secs(1);
for attempt in 0..5 {
    match call_api().await {
        Ok(r) => break r,
        Err(e) if e.status() == 429 => {
            tokio::time::sleep(delay).await;
            delay *= 2;
        }
        Err(e) => return Err(e),
    }
}
```

### Pièges
- Token limit : `max_tokens` + prompt > context window → tronqué
- Streaming : les chunks SSE arrivent dans le désordre parfois
- Pricing : gpt-4o = $2.50/1M input, $10/1M output
