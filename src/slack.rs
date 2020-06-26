use serde_json::{json, Value};

/// https://slack.com/api/chat.postMessage
pub fn chat_post_message(token: &str, channel: &str, blocks: Value) {
    let client = reqwest::blocking::Client::new();
    let res = client
        .post("https://slack.com/api/chat.postMessage")
        .bearer_auth(token)
        .json(&json!({"channel":channel, "blocks":blocks}))
        .send()
        .unwrap();
    res.error_for_status_ref().unwrap();
    println!("{:?}", res.json::<Value>().unwrap());
}
