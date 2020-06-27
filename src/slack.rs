use serde_json::{json, Value};

#[derive(Debug)]
pub enum SlackError {
    HttpError(reqwest::Error),
}

impl std::convert::From<reqwest::Error> for SlackError {
    fn from(e: reqwest::Error) -> Self {
        Self::HttpError(e)
    }
}

/// https://slack.com/api/chat.postMessage
pub fn chat_post_message(token: &str, channel: &str, blocks: Value) -> Result<(), SlackError> {
    let client = reqwest::blocking::Client::new();
    let res = client
        .post("https://slack.com/api/chat.postMessage")
        .bearer_auth(token)
        .json(&json!({"channel":channel, "blocks":blocks}))
        .send()?;
    res.error_for_status_ref()?;
    Ok(())
}
