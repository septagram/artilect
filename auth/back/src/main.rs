#[tokio::main]
async fn main() {
    auth_back::serve().await.unwrap();
}
