use actix_web::{web, App, HttpResponse, HttpServer, Responder, get, HttpRequest};
use actix_cors::Cors;
use handlebars::Handlebars;
use include_dir::{include_dir, Dir};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use crate::chat::Chat;
use crate::inference::types::{Message, Role, ContentItem, InferenceError};
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

#[derive(Serialize)]
pub struct ErrorResponse {
    error: Value,  // Changed to Value to support both string and object errors
    error_type: String,
    status_code: u16,
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

fn parse_error_message(message: &str) -> Value {
    // Try to parse the error message as JSON first
    if let Ok(json_value) = serde_json::from_str::<Value>(message) {
        json_value
    } else {
        // If it's not valid JSON, return it as a simple string
        Value::String(message.to_string())
    }
}

fn handle_inference_error(error: InferenceError) -> HttpResponse {
    match error {
        InferenceError::NetworkError(msg) => {
            HttpResponse::ServiceUnavailable().json(ErrorResponse {
                error: parse_error_message(&msg),
                error_type: "network_error".to_string(),
                status_code: 503,
            })
        }
        InferenceError::ApiError(status, msg) => {
            HttpResponse::build(status).json(ErrorResponse {
                error: parse_error_message(&msg),
                error_type: "api_error".to_string(),
                status_code: status.as_u16(),
            })
        }
        InferenceError::InvalidResponse(msg) => {
            HttpResponse::BadGateway().json(ErrorResponse {
                error: parse_error_message(&msg),
                error_type: "invalid_response".to_string(),
                status_code: 502,
            })
        }
        InferenceError::MissingApiKey(msg) => {
            HttpResponse::InternalServerError().json(ErrorResponse {
                error: parse_error_message(&msg),
                error_type: "configuration_error".to_string(),
                status_code: 500,
            })
        }
        InferenceError::SerializationError(msg) => {
            HttpResponse::InternalServerError().json(ErrorResponse {
                error: parse_error_message(&msg),
                error_type: "serialization_error".to_string(),
                status_code: 500,
            })
        }
    }
}

#[get("/messages")]
async fn get_messages(data: web::Data<AppState>) -> impl Responder {
    let chat = data.chat.lock().unwrap();
    HttpResponse::Ok().json(&chat.messages)
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

/*
    * The handler works by bouncing messages back and forth from client in a sequential manner.
    *
    * When new text input from client arrives, message is sent to third party API and response
    * then forwared to client.  If the client receives a tool_use (or tool_call for DeepSeek) then
    * it will immediately send back a tool_use message and this handler will NOT query third party
    * API and instead handle the tool use.  The tool_result is then send back to client, processed
    * by client and immediately sent back to third party API.  That response is then forwarded to
    * client and this continues until there are no more tool_use messages.
    *
    * The messages coming from client are of type Message but are guaranteed to only have a single
    * content item.
    *
*/
async fn chat_handler(
    data: web::Data<AppState>, 
    req: web::Json<ChatRequest>
) -> impl Responder {
    let mut chat = data.chat.lock().unwrap();

    match &req.0.message.content[0] {
        ContentItem::Text { .. } => {
            let new_msg = Message {
                role: Role::User,
                content: vec![req.0.message.content[0].clone()]
            };

            match chat.send_message(new_msg).await {
                Ok(returned_msg) => {
                    HttpResponse::Ok().json(ChatResponse {
                        message: returned_msg,
                    })
                },
                Err(e) => {
                    match e.downcast::<InferenceError>() {
                        Ok(inference_error) => handle_inference_error(inference_error),
                        Err(other_error) => HttpResponse::InternalServerError().json(ErrorResponse {
                            error: parse_error_message(&other_error.to_string()),
                            error_type: "unknown_error".to_string(),
                            status_code: 500,
                        })
                    }
                }
            }
        },
        ContentItem::ToolUse { id, .. } => {
            match chat.handle_tool_use(&req.0.message.content[0]).await {
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
                Err(e) => {
                    HttpResponse::InternalServerError().json(ErrorResponse {
                        error: parse_error_message(&e.to_string()),
                        error_type: "tool_error".to_string(),
                        status_code: 500,
                    })
                }
            }
        },
        ContentItem::ToolResult { .. } => {
            let msg = Message {
                role: Role::User,
                content: req.0.message.content.clone(),
            };
            
            match chat.send_message(msg).await {
                Ok(returned_msg) => {
                    HttpResponse::Ok().json(ChatResponse {
                        message: returned_msg,
                    })
                },
                Err(e) => {
                    match e.downcast::<InferenceError>() {
                        Ok(inference_error) => handle_inference_error(inference_error),
                        Err(other_error) => HttpResponse::InternalServerError().json(ErrorResponse {
                            error: parse_error_message(&other_error.to_string()),
                            error_type: "unknown_error".to_string(),
                            status_code: 500,
                        })
                    }
                }
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
            .service(get_messages)
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
