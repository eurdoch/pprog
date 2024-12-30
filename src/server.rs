use actix_web::{web, App, HttpResponse, HttpServer, Responder, get, HttpRequest};
use actix_cors::Cors;
use serde::{Deserialize, Serialize};
use crate::chat::Chat;
use crate::inference::{Message, Role, ContentItem};
use std::sync::Mutex;
use std::collections::HashMap;
use actix_web::http;

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

// Macro to include all frontend files at compile time
macro_rules! include_frontend_files {
    () => {{
        let mut files: HashMap<String, &'static [u8]> = HashMap::new();
        
        // Directly include files from the dist directory
        files.insert("index.html".to_string(), include_bytes!("../frontend/dist/index.html"));
        files.insert("assets/index-BD9teqBM.css".to_string(), include_bytes!("../frontend/dist/assets/index-BD9teqBM.css"));
        files.insert("assets/index-BkiH-5Ms.js".to_string(), include_bytes!("../frontend/dist/assets/index-BkiH-5Ms.js"));
        files.insert("vite.svg".to_string(), include_bytes!("../frontend/dist/vite.svg"));

        files
    }};
}

fn get_mime_type(filename: &str) -> &'static str {
    match filename {
        f if f.ends_with(".html") => "text/html; charset=utf-8",
        f if f.ends_with(".css") => "text/css; charset=utf-8",
        f if f.ends_with(".js") => "application/javascript; charset=utf-8",
        f if f.ends_with(".svg") => "image/svg+xml",
        _ => "application/octet-stream"
    }
}

#[get("/{filename:.*}")]
async fn index(req: HttpRequest, static_files: web::Data<HashMap<String, &'static [u8]>>) -> impl Responder {
    let path = req.match_info().query("filename").to_string();
    let path_to_check = if path.is_empty() { "index.html".to_string() } else { path };

    match static_files.get(&path_to_check) {
        Some(contents) => {
            HttpResponse::Ok()
                .content_type(get_mime_type(&path_to_check))
                .body(contents.to_vec())
        },
        None => {
            // Fallback to index.html for client-side routing
            let index_html = static_files.get("index.html").unwrap();
            HttpResponse::Ok()
                .content_type("text/html; charset=utf-8")
                .body(index_html.to_vec())
        }
    }
}

async fn chat_handler(
    data: web::Data<AppState>, 
    req: web::Json<ChatRequest>
) -> impl Responder {
    let mut _chat = data.chat.lock().unwrap();

    match &req.message.content[0] {
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
                        
                    HttpResponse::Ok().json(ChatResponse {
                        message: ai_message,
                    })
                },
                Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": format!("Error: {}", e)
                }))
            }
        },
        _ => HttpResponse::InternalServerError().body("Unhandled message type")
    }
}

pub async fn start_server(host: String, port: u16) -> std::io::Result<()> {
    // Embed frontend files
    let static_files = include_frontend_files!();

    let app_state = web::Data::new(AppState {
        chat: Mutex::new(Chat::new()),
    });

    println!("Starting server on {}:{}", host, port);

    HttpServer::new(move || {
        let cors = Cors::default()
            .allowed_origin("http://localhost:5173")
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
            .app_data(web::Data::new(static_files.clone()))
            .route("/chat", web::post().to(chat_handler))
            .service(index)
    })
    .bind(format!("{}:{}", host, port))?
    .run()
    .await
}
