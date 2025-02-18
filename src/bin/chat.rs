#![feature(let_chains)]

#[tokio::main]
async fn main() {
    // Initialize logging
    tracing_subscriber::fmt::init();

    // Load configuration
    dotenvy::dotenv().ok();
    let name: Box<str> = std::env::var("NAME")
        .expect("NAME must be set")
        .trim()
        .into();

    if name.is_empty() {
        panic!("NAME cannot be empty");
    }

    let database_url = std::env::var("CHAT_DATABASE_URL").expect("DATABASE_URL must be set");
    let port = match std::env::var("PORT") {
        Ok(port) => Some(port.parse::<u16>().expect("Invalid PORT")),
        Err(VarError::NotPresent) => None,
        Err(err) => panic!("Failed to parse PORT: {}", err),
    };

    chat_back::serve(name, database_url, port).await.unwrap();
}
