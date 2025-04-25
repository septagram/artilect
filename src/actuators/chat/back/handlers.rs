use actix::prelude::*;
use axum::{
    extract::{Path, State},
    http::{HeaderValue, Method, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Router,
    Json,
};
use axum_extra::TypedHeader;
use dioxus_lib::prelude::*;
use headers::authorization::{Authorization, Bearer};
use tower_http::cors::CorsLayer;
use crate::infer::{PlainText, infer_value};
use serde_json::json;
use sqlx::PgPool;
use std::ops::Deref;
use std::sync::Arc;
use uuid::Uuid;
use crate::service;

// use crate::auth::dto::{Authenticated, Identity};
use crate::actuators::chat::dto::{
    FetchThreadRequest, FetchThreadResponse, FetchUserThreadsRequest, FetchUserThreadsResponse, ChatMessage, OneToManyChild, OneToManyUpdate,
    SendMessageRequest, SendMessageResponse, SyncUpdate, Thread, User,
};

use super::{actor::ChatService, components::message_log::MessageLogItem, components::MessageLog};

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

fn map_service_response<T>(actix_response: Result<service::Result<T>, MailboxError>) -> service::Result<Json<T>> { //Result<Json<T>, StatusCode> {
    match actix_response {
        Ok(service_response) => match service_response {
            Ok(response) => Ok(Json(response)),
            Err(error) => Err(error),
        }
        Err(_) => Err(service::Error::ServiceUnavailable),
    }
}

pub async fn fetch_user_threads_handler(
    State(service): State<Arc<Addr<ChatService>>>,
    auth_header: TypedHeader<Authorization<Bearer>>,
) -> service::Result<Json<FetchUserThreadsResponse>> {
    let from_user_id = Uuid::parse_str(&auth_header.token()).map_err(|_| service::Error::Unauthorized)?;
    map_service_response(service.send(FetchUserThreadsRequest { from_user_id }).await)
}

pub async fn fetch_thread_handler(
    State(service): State<Arc<Addr<ChatService>>>,
    auth_header: TypedHeader<Authorization<Bearer>>,
    Path(thread_id): Path<Uuid>,
) -> service::Result<Json<FetchThreadResponse>> {
    let from_user_id = Uuid::parse_str(&auth_header.token()).map_err(|_| service::Error::Unauthorized)?;
    map_service_response(service.send(FetchThreadRequest { from_user_id, thread_id }).await)
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
        map_service_response(service.send(request).await)
    }
}
