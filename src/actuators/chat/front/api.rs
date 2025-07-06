use crate::actuators::chat::dto::{
    FetchThreadResponse, FetchUserThreadsResponse, ChatMessage, SendMessageRequest, SendMessageResponse,
};
use reqwest::Client;
use std::error::Error;
use uuid::{Uuid, uuid};

static BASE_URL: &str = dotenvy_macro::dotenv!("CHAT_BASE_URL");
static USER_ID: Uuid = uuid!(dotenvy_macro::dotenv!("CHAT_USER_ID"));

pub async fn fetch_user_threads() -> Result<FetchUserThreadsResponse, Box<dyn Error>> {
    let client = Client::new();
    let response = client
        .get(format!("{BASE_URL}/chats"))
        .header("Authorization", format!("Bearer {USER_ID}"))
        .send()
        .await?;
    Ok(response.json::<FetchUserThreadsResponse>().await?)
}

pub async fn fetch_thread(thread_id: Uuid) -> Result<FetchThreadResponse, Box<dyn Error>> {
    let client = Client::new();
    let response = client
        .get(format!("{BASE_URL}/chat/{thread_id}"))
        .header("Authorization", format!("Bearer {USER_ID}"))
        .send()
        .await?;
    Ok(response.json::<FetchThreadResponse>().await?)
}

pub async fn send_message(
    message: &ChatMessage,
    is_new_thread: bool,
) -> Result<SendMessageResponse, Box<dyn Error>> {
    let client = Client::new();
    match client
        .post(format!("{BASE_URL}/chat"))
        .header("Authorization", format!("Bearer {USER_ID}"))
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
