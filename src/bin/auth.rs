#[tokio::main]
async fn main() {
    // Initialize logging
    tracing_subscriber::fmt::init();

    artilect::auth::back::serve().await.unwrap();
}
