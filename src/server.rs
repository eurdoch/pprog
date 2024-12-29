use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use serde::{Deserialize, Serialize};
use crate::chat::Chat;
use crate::inference::{Inference, Message, Role, ContentItem};
use std::sync::Mutex;

#[derive(Deserialize)]
pub struct ChatRequest {
    message: String,
}

#[derive(Serialize)]
pub struct ChatResponse {
    response: String,
}

pub struct AppState {
    chat: Mutex<Chat>,
    inference: Mutex<Inference>,
}

async fn chat_handler(
    data: web::Data<AppState>, 
    req: web::Json<ChatRequest>
) -> impl Responder {
    let _chat = data.chat.lock().unwrap();
    let inference = data.inference.lock().unwrap();

    // Add user message to chat
    let _user_message = Message {
        role: Role::User,
        content: vec![ContentItem::Text { text: req.message.clone() }]
    };

    // Use generate_response async
    match inference.generate_response(&req.message).await {
        Ok(ai_response) => {
            // Add AI response to chat
            let _ai_message = Message {
                role: Role::Assistant,
                content: vec![ContentItem::Text { text: ai_response.clone() }]
            };
            
            HttpResponse::Ok().json(ChatResponse {
                response: ai_response,
            })
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

pub async fn start_server(inference: Inference, host: String, port: u16) -> std::io::Result<()> {
    let app_state = web::Data::new(AppState {
        chat: Mutex::new(Chat::new()),
        inference: Mutex::new(inference),
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
