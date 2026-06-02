use sparrow::gateway::MessageRouter;

#[test]
fn gateway_session_key_scopes_surface_channel_and_peer() {
    let a = MessageRouter::session_key("u1", "telegram", "chat-a");
    let b = MessageRouter::session_key("u1", "telegram", "chat-b");
    let c = MessageRouter::session_key("u1", "slack", "chat-a");
    assert_ne!(a, b);
    assert_ne!(a, c);
    assert!(a.contains("gateway:telegram:channel:chat-a:peer:u1"));
}

#[test]
fn gateway_session_key_sanitizes_empty_and_punctuated_parts() {
    let key = MessageRouter::session_key("", "web socket", "room/42");
    assert_eq!(key, "gateway:web_socket:channel:room_42:peer:anonymous");
}
