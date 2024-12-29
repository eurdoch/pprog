use actix_web::{web, App, HttpServer, HttpResponse, Responder};
use serde::{Serialize, Deserialize};
use std::sync::Mutex;
use anyhow::Result;
use crate::{chat::Chat, inference::{Message, ContentItem, Role}};

pub struct ServerState {
    chat: Mutex<Chat>,
}

#[derive(Debug, Serialize)]
struct ApiResponse {
    status: String,
    message: String,
}

#[derive(Debug, Deserialize)]
struct ChatRequest {
    message: String,
}

#[derive(Debug, Serialize)]
struct ChatResponse {
    messages: Vec<Message>,
}

pub async fn start_server(host: String, port: u16) -> Result<()> {
    let state = web::Data::new(ServerState {
        chat: Mutex::new(Chat::new()),
    });

    println!("Starting server at http://{}:{}", host, port);
    
    HttpServer::new(move || {
        App::new()
            .app_data(state.clone())
            .service(web::scope("/api")
                .route("/health", web::get().to(health_check))
                .route("/chat", web::post().to(handle_chat))
                .route("/chat/history", web::get().to(get_chat_history)))
    })
    .bind((host, port))?
    .run()
    .await?;

    Ok(())
}

async fn health_check() -> impl Responder {
    let response = ApiResponse {
        status: "ok".to_string(),
        message: "Server is running".to_string(),
    };
    HttpResponse::Ok().json(response)
}

async fn handle_chat(
    state: web::Data<ServerState>,
    request: web::Json<ChatRequest>,
) -> impl Responder {
    let mut chat = state.chat.lock().unwrap();
    
    let user_message = Message {
        role: Role::User,
        content: vec![ContentItem::Text { 
            text: request.message.clone() 
        }],
    };

    match chat.add_message(user_message).await {
        Ok(_) => {
            let response = ChatResponse {
                messages: chat.messages.clone(),
            };
            HttpResponse::Ok().json(response)
        },
        Err(e) => {
            HttpResponse::InternalServerError().json(ApiResponse {
                status: "error".to_string(),
                message: format!("Failed to process chat message: {}", e),
            })
        }
    }
}

async fn get_chat_history(state: web::Data<ServerState>) -> impl Responder {
    let chat = state.chat.lock().unwrap();
    let response = ChatResponse {
        messages: chat.messages.clone(),
    };
    HttpResponse::Ok().json(response)
}
