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

use super::actor::ChatPrecept;
use crate::{
    actuators::chat::dto::{
        FetchThreadRequest, FetchThreadResponse, FetchUserThreadsRequest, FetchUserThreadsResponse,
        SendMessageRequest, SendMessageResponse,
    },
    precept,
    precept::{ActixResult, SignedMessage, Identity},
};

pub fn build_router(state: Arc<Addr<ChatPrecept>>) -> Router {
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
    State(precept): State<Arc<Addr<ChatPrecept>>>,
    auth_header: TypedHeader<Authorization<Bearer>>,
) -> precept::Result<Json<FetchUserThreadsResponse>> {
    let user_id = Uuid::parse_str(&auth_header.token()).map_err(|_| precept::Error::Unauthorized)?;
    precept
        .send(SignedMessage {
            from: Identity {
                user_id,
                precept_id: None,
            },
            data: FetchUserThreadsRequest {},
        })
        .await
        .into_precept_result()
        .map(|response| Json(response))
}

pub async fn fetch_thread_handler(
    State(precept): State<Arc<Addr<ChatPrecept>>>,
    auth_header: TypedHeader<Authorization<Bearer>>,
    Path(thread_id): Path<Uuid>,
) -> precept::Result<Json<FetchThreadResponse>> {
    let user_id = Uuid::parse_str(&auth_header.token()).map_err(|_| precept::Error::Unauthorized)?;
    precept
        .send(SignedMessage {
            from: Identity {
                user_id,
                precept_id: None,
            },
            data: FetchThreadRequest { thread_id },
        })
        .await
        .into_precept_result()
        .map(|response| Json(response))
}

pub async fn chat_handler(
    State(precept): State<Arc<Addr<ChatPrecept>>>,
    auth_header: TypedHeader<Authorization<Bearer>>,
    Json(request): Json<SendMessageRequest>,
) -> precept::Result<Json<SendMessageResponse>> {
    let user_id = Uuid::parse_str(&auth_header.token()).map_err(|_| precept::Error::Unauthorized)?;
    precept
        .send(SignedMessage {
            from: Identity {
                user_id,
                precept_id: None,
            },
            data: request,
        })
        .await
        .into_precept_result()
        .map(|response| Json(response))
}
