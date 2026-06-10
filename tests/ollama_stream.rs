use futures::StreamExt;
use sparrow::event::{StopReason, TokenUsage};
use sparrow::provider::ollama::OllamaAdapter;
use sparrow::provider::{Brain, BrainEvent, BrainRequest, LatencyClass, ModelCaps, ToolSpec};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

async fn write_json_response(socket: &mut tokio::net::TcpStream, body: &str, content_type: &str) {
    let response = format!(
        "HTTP/1.1 200 OK\r\ncontent-type: {}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
        content_type,
        body.len(),
        body
    );
    socket.write_all(response.as_bytes()).await.unwrap();
}

#[tokio::test]
async fn ollama_ndjson_stream_maps_text_usage_and_done() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut show_socket, _) = listener.accept().await.unwrap();
        let mut show_buf = [0_u8; 4096];
        let _ = show_socket.read(&mut show_buf).await.unwrap();
        write_json_response(
            &mut show_socket,
            "{\"capabilities\":[\"completion\",\"tools\"],\"model_info\":{\"llama.context_length\":8192}}",
            "application/json",
        )
        .await;

        let (mut socket, _) = listener.accept().await.unwrap();
        let mut buf = [0_u8; 4096];
        let _ = socket.read(&mut buf).await.unwrap();
        let body = concat!(
            "{\"message\":{\"content\":\"hello\"},\"done\":false}\n",
            "{\"prompt_eval_count\":3,\"eval_count\":2,\"done\":true,\"done_reason\":\"stop\"}\n"
        );
        write_json_response(&mut socket, body, "application/x-ndjson").await;
    });

    let adapter = OllamaAdapter::new("qwen-test", &format!("http://{}", addr));
    let mut stream = adapter.complete(BrainRequest::default()).await.unwrap();

    let mut text = String::new();
    let mut usage = None;
    let mut done = None;
    while let Some(event) = stream.next().await {
        match event {
            BrainEvent::TextDelta(delta) => text.push_str(&delta),
            BrainEvent::Usage(u) => usage = Some(u),
            BrainEvent::Done(reason) => done = Some(reason),
            other => panic!("unexpected event: {other:?}"),
        }
    }

    server.await.unwrap();
    assert_eq!(text, "hello");
    assert!(matches!(
        usage,
        Some(TokenUsage {
            input: 3,
            output: 2
        })
    ));
    assert!(matches!(done, Some(StopReason::EndTurn)));
}

#[tokio::test]
async fn ollama_omits_tools_when_model_caps_disable_them() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (tx, rx) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        let (mut show_socket, _) = listener.accept().await.unwrap();
        let mut show_buf = vec![0_u8; 8192];
        let _ = show_socket.read(&mut show_buf).await.unwrap();
        write_json_response(
            &mut show_socket,
            "{\"capabilities\":[\"completion\"],\"model_info\":{\"llama.context_length\":4096}}",
            "application/json",
        )
        .await;

        let (mut socket, _) = listener.accept().await.unwrap();
        let mut buf = vec![0_u8; 8192];
        let n = socket.read(&mut buf).await.unwrap();
        let request = String::from_utf8_lossy(&buf[..n]).to_string();
        tx.send(request).unwrap();
        let body = "{\"done\":true,\"done_reason\":\"stop\"}\n";
        write_json_response(&mut socket, body, "application/x-ndjson").await;
    });

    let adapter =
        OllamaAdapter::new("tiny-local", &format!("http://{}", addr)).with_caps(ModelCaps {
            context_window: 4_096,
            max_output: 1_024,
            tools: false,
            vision: false,
            cost_input_per_mtok: 0.0,
            cost_output_per_mtok: 0.0,
            latency: LatencyClass::Fast,
        });
    let req = BrainRequest {
        tools: vec![ToolSpec {
            name: "read".into(),
            description: "read a file".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "file_path": {"type": "string"}
                }
            }),
        }],
        ..BrainRequest::default()
    };

    let mut stream = adapter.complete(req).await.unwrap();
    while stream.next().await.is_some() {}
    server.await.unwrap();

    let request = rx.await.unwrap();
    let body = request.split("\r\n\r\n").nth(1).unwrap();
    let json: serde_json::Value = serde_json::from_str(body).unwrap();
    assert!(
        json.get("tools").is_none(),
        "tool schema must not be sent to Ollama models without tool support: {json}"
    );
    assert_eq!(json["options"]["num_ctx"], 4096);
}
