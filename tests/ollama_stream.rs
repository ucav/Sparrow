use futures::StreamExt;
use sparrow::event::{StopReason, TokenUsage};
use sparrow::provider::ollama::OllamaAdapter;
use sparrow::provider::{Brain, BrainEvent, BrainRequest};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

#[tokio::test]
async fn ollama_ndjson_stream_maps_text_usage_and_done() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut buf = [0_u8; 4096];
        let _ = socket.read(&mut buf).await.unwrap();
        let body = concat!(
            "{\"message\":{\"content\":\"hello\"},\"done\":false}\n",
            "{\"prompt_eval_count\":3,\"eval_count\":2,\"done\":true,\"done_reason\":\"stop\"}\n"
        );
        let response = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: application/x-ndjson\r\ncontent-length: {}\r\n\r\n{}",
            body.len(),
            body
        );
        socket.write_all(response.as_bytes()).await.unwrap();
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
