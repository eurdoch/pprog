use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use serde::{Deserialize, Serialize};
use crate::chat::Chat;
use crate::inference::{Message, Role, ContentItem};
use std::sync::Mutex;

#[derive(Deserialize)]
pub struct ChatRequest {
    message: String,
}

// Modify to include full AnthropicResponse
#[derive(Serialize, Clone)]
pub struct ChatResponse {
    message: Message,
}

pub struct AppState {
    chat: Mutex<Chat>,
}

async fn chat_handler(
    data: web::Data<AppState>, 
    req: web::Json<ChatRequest>
) -> impl Responder {
    let mut _chat = data.chat.lock().unwrap();

    // Create user message with full content
    let user_message = Message {
        role: Role::User,
        content: vec![ContentItem::Text { text: req.message.clone() }]
    };

    _chat.messages.push(user_message.clone());
    match _chat.send_message(user_message).await {
        Ok(response_option) => {
            if let Some(response) = response_option {
                let ai_message = Message {
                    role: Role::Assistant,
                    content: response.content.clone()
                };
                
                return HttpResponse::Ok().json(ChatResponse {
                    message: ai_message,
                })
            } else {
                return HttpResponse::InternalServerError().body(format!("NetworkError"))
            }
        },
        Err(e) => HttpResponse::InternalServerError().body(format!("Error: {}", e))
    }
}

async fn get_chat_history(
    data: web::Data<AppState>
) -> impl Responder {
    let chat = data.chat.lock().unwrap();
    
    HttpResponse::Ok().json(chat.messages.clone())
}

pub async fn start_server(host: String, port: u16) -> std::io::Result<()> {
    let app_state = web::Data::new(AppState {
        chat: Mutex::new(Chat::new()),
    });

    println!("Starting server on {}:{}", host, port);

    HttpServer::new(move || {
        App::new()
            .app_data(app_state.clone())
            .route("/chat", web::post().to(chat_handler))
            .route("/history", web::get().to(get_chat_history))
    })
    .bind(format!("{}:{}", host, port))?
    .run()
    .await
}
