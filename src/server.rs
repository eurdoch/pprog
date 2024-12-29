use actix_web::{web, App, HttpResponse, HttpServer, Responder, get, HttpRequest};
use actix_cors::Cors;
use serde::{Deserialize, Serialize};
use crate::chat::Chat;
use crate::inference::{Message, Role, ContentItem};
use std::sync::Mutex;
use include_dir::{include_dir, Dir};
use mime_guess::from_path;
use actix_web::http;

static FRONTEND_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/frontend/dist");

#[derive(Deserialize)]
pub struct ChatRequest {
    message: Message,
}

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

    // TODO start over, assume only one content item
    match &req.message.content[0] {
        // Normal text query from user
        ContentItem::Text { .. } => {
            let new_msg = Message {
                role: Role::User,
                content: vec![req.message.content[0].clone()]
            };
            _chat.messages.push(new_msg.clone());
            match _chat.send_message(new_msg).await {
                Ok(response) => {
                    let ai_message = Message {
                        role: Role::Assistant,
                        content: response.content.clone()
                    };
                    _chat.messages.push(ai_message.clone());
                        
                    return HttpResponse::Ok().json(ChatResponse {
                        message: ai_message,
                    })
                },
                Err(e) => return HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": format!("Error: {}", e)
                }))
            }
        },
        // A user has received tool use msg and immediately returned to get result
        ContentItem::ToolUse { id, .. } => {
            match _chat.handle_tool_use(&req.message.content[0]).await {
                Ok(tool_result_content) => {
                    let message = Message {
                        role: Role::User,
                        content: vec![ContentItem::ToolResult {
                            tool_use_id: id.to_string(),
                            content: tool_result_content,
                        }],
                    };
                    return HttpResponse::Ok().json(ChatResponse {
                        message
                    })
                },
                Err(e) => {
                    return HttpResponse::InternalServerError().json(serde_json::json!({
                        "error": format!("Error: {}", e)
                    }))
                }
            }
        },
        ContentItem::ToolResult { .. } => {
            let msg = Message {
                role: Role::User,
                content: vec![req.message.content[0].clone()]
            };
            match _chat.send_message(msg).await {
                Ok(response) => {
                    let ai_message = Message {
                        role: Role::Assistant,
                        content: response.content.clone()
                    };
                    _chat.messages.push(ai_message.clone());
                        
                    return HttpResponse::Ok().json(ChatResponse {
                        message: ai_message,
                    })
                },
                Err(e) => return HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": format!("Error: {}", e)
                }))
            }
        },
    }
}

async fn get_chat_history(
    data: web::Data<AppState>
) -> impl Responder {
    let chat = data.chat.lock().unwrap();
    
    HttpResponse::Ok().json(chat.messages.clone())
}

#[get("/{filename:.*}")]
async fn index(req: HttpRequest) -> impl Responder {
    let path = req.match_info().query("filename").to_string();
    let path = if path.is_empty() { "index.html".to_string() } else { path };

    // Try to get the file from the embedded directory
    if let Some(file) = FRONTEND_DIR.get_file(&path) {
        // Guess the mime type
        let mime_type = from_path(&path).first_or_octet_stream();
        
        return HttpResponse::Ok()
            .content_type(mime_type.as_ref())
            .body(file.contents());
    }

    // If file not found, serve index.html for client-side routing
    if let Some(index_file) = FRONTEND_DIR.get_file("index.html") {
        HttpResponse::Ok()
            .content_type("text/html")
            .body(index_file.contents())
    } else {
        HttpResponse::InternalServerError()
            .body("index.html not found in embedded files")
    }
}

pub async fn start_server(host: String, port: u16) -> std::io::Result<()> {
    let app_state = web::Data::new(AppState {
        chat: Mutex::new(Chat::new()),
    });

    println!("Starting server on {}:{}", host, port);

    HttpServer::new(move || {
        let cors = Cors::default()
            .allowed_origin("http://localhost:5173")  // React Vite default dev server
            .allowed_origin("http://127.0.0.1:5173")
            .allowed_methods(vec!["GET", "POST", "OPTIONS"])
            .allowed_headers(vec![
                http::header::AUTHORIZATION, 
                http::header::ACCEPT, 
                http::header::CONTENT_TYPE
            ])
            .supports_credentials()
            .max_age(3600);

        App::new()
            .wrap(cors)
            .app_data(app_state.clone())
            .route("/chat", web::post().to(chat_handler))
            .route("/history", web::get().to(get_chat_history))
            .service(index)
    })
    .bind(format!("{}:{}", host, port))?
    .run()
    .await
}
