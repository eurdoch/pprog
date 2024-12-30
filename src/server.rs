use actix_web::{web, App, HttpResponse, HttpServer, Responder, get, HttpRequest};
use actix_cors::Cors;
use handlebars::Handlebars;
use include_dir::{include_dir, Dir};
use serde::{Deserialize, Serialize};
use serde_json::json;
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
    static_files: HashMap<String, Vec<u8>>,
}

static DIST_DIR: Dir = include_dir!("./frontend/dist/");

fn get_mime_type(filename: &str) -> &'static str {
    match filename {
        f if f.ends_with(".html") => "text/html; charset=utf-8",
        f if f.ends_with(".css") => "text/css; charset=utf-8",
        f if f.ends_with(".js") => "application/javascript; charset=utf-8",
        f if f.ends_with(".svg") => "image/svg+xml",
        _ => "application/octet-stream"
    }
}

#[get("/clear")]
async fn clear_chat(data: web::Data<AppState>) -> impl Responder {
    let mut chat = data.chat.lock().unwrap();
    let system_prompt = chat.messages.first().filter(|msg| msg.role == Role::System).cloned();
    chat.messages.clear();
    if let Some(prompt) = system_prompt {
        chat.messages.push(prompt);
    }
    HttpResponse::Ok().json(json!({"cleared": true, "message": "Chat history cleared"}))
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
        ContentItem::ToolUse { id, .. } => {
            match _chat.handle_tool_use(&req.message.content[0]).await {
                Ok(tool_use_result) => HttpResponse::Ok().json(ChatResponse {
                    message: Message {
                        role: Role::User,
                        content: vec![
                            ContentItem::ToolResult {
                                tool_use_id: id.to_string(),
                                content: tool_use_result
                            }
                        ]

                    }
                }),
                Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": format!("Error: {}", e)
                }))
            }
        },
        ContentItem::ToolResult { .. } => {
            let msg = Message {
                role: Role::User,
                content: req.message.content.clone(),
            };
            _chat.messages.push(msg.clone());
            match _chat.send_message(msg).await {
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
        }
    }
}

fn process_files(dir: &Dir, base_path: &str, static_files: &mut HashMap<String, Vec<u8>>, hbs: &mut Handlebars, template_data: &serde_json::Value) {
    for entry in dir.entries() {
        let relative_path = entry.path().to_string_lossy().replace("\\", "/");
        let full_path = format!("{}/{}", base_path, relative_path);

        if let Some(subdir) = entry.as_dir() {
            process_files(subdir, &full_path, static_files, hbs, template_data);
        } else if let Some(file) = entry.as_file() {
            let contents = if full_path.ends_with(".html") {
                let template_str = String::from_utf8_lossy(file.contents());
                // Register and render the template using Handlebars
                match hbs.render_template(&template_str, template_data) {
                    Ok(rendered) => rendered.into_bytes(),
                    Err(_) => file.contents().to_vec()
                }
            } else {
                file.contents().to_vec()
            };

            // Normalize path and ensure it starts with a slash
            let normalized_path = if full_path.starts_with('/') { full_path.clone() } else { format!("/{}", full_path) };
            
            static_files.insert(normalized_path, contents);
        }
    }
}

pub async fn start_server(host: String, port: u16) -> std::io::Result<()> {
    let server_url = format!("http://{}:{}", host, port);
    let template_data = json!({
        "server_url": server_url
    });

    let mut hbs = Handlebars::new();
    let mut static_files = HashMap::new();
    
    process_files(&DIST_DIR, "", &mut static_files, &mut hbs, &template_data);

    let app_state = web::Data::new(AppState {
        chat: Mutex::new(Chat::new()),
        static_files,
    });

    println!("Starting server on {}:{}", host, port);

    HttpServer::new(move || {
        let cors = Cors::default()
            // TODO find a way to make this hostable on server
            .allowed_origin(&server_url)
            .allowed_origin(&format!("http://localhost:{}", port))
            .allowed_origin(&format!("http://127.0.0.1:{}", port))
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
            .service(clear_chat)
            .service(index)
    })
    .bind(format!("{}:{}", host, port))?
    .run()
    .await
}


#[get("/{filename:.*}")]
async fn index(
    req: HttpRequest, 
    app_data: web::Data<AppState>
) -> impl Responder {
    let path = req.match_info().query("filename").to_string();

    // Handle root path (index.html)
    if path.is_empty() {
        return match app_data.as_ref().static_files.get("/index.html") {
            Some(contents) => HttpResponse::Ok()
                .content_type("text/html; charset=utf-8")
                .body(contents.to_vec()),
            None => HttpResponse::NotFound().body("Index file not found"),
        };
    }

    // Try different path variations
    let path_variations = vec![
        format!("/{}", path),
        path.clone(),
        format!("/assets/{}", path),
        format!("assets/{}", path)
    ];

    for variation in path_variations {
        if let Some(contents) = app_data.as_ref().static_files.get(&variation) {
            let content_type = get_mime_type(&path);
            return HttpResponse::Ok()
                .content_type(content_type)
                .body(contents.to_vec());
        }
    }

    HttpResponse::NotFound().body(format!("File not found: {}", path))
}
