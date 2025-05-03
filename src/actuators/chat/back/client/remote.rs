pub struct ChatClient {}

impl super::ChatClientTrait for ChatClient {
    async fn fetch_user_threads(self: &Self, request: FetchUserThreadsRequest) -> service::Result<FetchUserThreadsResponse>;
    async fn fetch_thread_messages(self: &Self, request: FetchThreadRequest) -> service::Result<FetchThreadResponse>;
    async fn chat(self: &Self, request: SendMessageRequest) -> service::Result<SendMessageResponse>;
}
