use serde_json::Value;

fn main() {
    let data = r#"{"response": {"candidates": [{"content": {"role": "model","parts": [{"text": "Pong"}]}}],"usageMetadata": {"promptTokenCount": 3,"candidatesTokenCount": 1,"totalTokenCount": 4},"modelVersion": "gemini-2.5-flash-lite","responseId": "D96oaY_SE8ni_uMPlPq08Q0"},"traceId": "90deb727cd5ca59a"}"#;
    let json: Value = serde_json::from_str(data).unwrap();
    let candidates = json.get("response").and_then(|r| r.get("candidates")).or_else(|| json.get("candidates"));
    let parts = candidates
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("content"))
        .and_then(|c| c.get("parts"))
        .and_then(|p| p.as_array())
        .unwrap();
    println!("Extracted parts: {:?}", parts);
}
