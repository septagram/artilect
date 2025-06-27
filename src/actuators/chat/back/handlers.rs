use std::sync::Arc;

use actix::prelude::*;
use axum::{
    Json, Router,
    extract::{Path, State},
    http::{HeaderValue, Method},
    routing::{get, post},
};
use axum_extra::TypedHeader;
use headers::authorization::{Authorization, Bearer};
use tower_http::cors::CorsLayer;
use uuid::Uuid;

use super::actor::ChatService;
use crate::{
    actuators::chat::dto::{
        FetchThreadRequest, FetchThreadResponse, FetchUserThreadsRequest, FetchUserThreadsResponse,
        SendMessageRequest, SendMessageResponse,
    },
    service,
    service::ActixResult,
};

pub fn build_router(state: Arc<Addr<ChatService>>) -> Router {
    // Configure CORS
    let cors = CorsLayer::new()
        .allow_origin("*".parse::<HeaderValue>().unwrap())
        .allow_methods([Method::GET, Method::POST])
        .allow_headers([http::header::AUTHORIZATION, http::header::CONTENT_TYPE]);

    // Build router
    Router::new()
        .route("/chats", get(fetch_user_threads_handler))
        .route("/chat/{thread_id}", get(fetch_thread_handler))
        .route("/chat", post(chat_handler))
        .layer(cors)
        .with_state(state)
}

pub async fn fetch_user_threads_handler(
    State(service): State<Arc<Addr<ChatService>>>,
    auth_header: TypedHeader<Authorization<Bearer>>,
) -> service::Result<Json<FetchUserThreadsResponse>> {
    let from_user_id = Uuid::parse_str(&auth_header.token()).map_err(|_| service::Error::Unauthorized)?;
    service.send(FetchUserThreadsRequest { from_user_id }).await.into_service_result()
}

pub async fn fetch_thread_handler(
    State(service): State<Arc<Addr<ChatService>>>,
    auth_header: TypedHeader<Authorization<Bearer>>,
    Path(thread_id): Path<Uuid>,
) -> service::Result<Json<FetchThreadResponse>> {
    let from_user_id = Uuid::parse_str(&auth_header.token()).map_err(|_| service::Error::Unauthorized)?;
    service.send(FetchThreadRequest { from_user_id, thread_id }).await.into_service_result()
}

pub async fn chat_handler(
    State(service): State<Arc<Addr<ChatService>>>,
    auth_header: TypedHeader<Authorization<Bearer>>,
    Json(request): Json<SendMessageRequest>,
) -> service::Result<Json<SendMessageResponse>> {
    let from_user_id = Uuid::parse_str(&auth_header.token()).map_err(|_| service::Error::Unauthorized)?;
    if from_user_id != request.from_user_id {
        Err(service::Error::Unauthorized)
    } else {
        service.send(request).await.into_service_result()
    }
}
