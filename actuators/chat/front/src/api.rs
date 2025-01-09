use reqwest::Client;
use std::error::Error;
use chat_dto::{Message, SendMessageRequest, SendMessageResponse};

static BASE_URL: &str = dotenvy_macro::dotenv!("CHAT_BASE_URL");

pub async fn send_message(
    message: &Message,
    is_new_thread: bool,
) -> Result<SendMessageResponse, Box<dyn Error>> {
    let client = Client::new();
    match client
        .post(format!("{BASE_URL}/chat"))
        .json(&SendMessageRequest {
            message: message.clone(),
            is_new_thread,
        })
        .send()
        .await
    {
        Ok(res) => {
            if let Ok(response) = res.json::<SendMessageResponse>().await {
                Ok(response)
            } else {
                Err("Failed to send message".into())
            }
        }
        Err(error) => Err(error.into()),
    }
}
