use actix_web::{web, App, HttpResponse, HttpServer, Responder, get, HttpRequest};
use actix_cors::Cors;
use serde::{Deserialize, Serialize};
use crate::chat::Chat;
use crate::inference::{Message, Role, ContentItem};
use std::sync::Mutex;
use std::path::{Path, PathBuf};
use std::fs;
use std::io;
use std::process::Command;
use mime_guess::from_path;
use home;
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

fn copy_dir_recursive(src: &Path, dst: &Path) -> io::Result<()> {
    // Create destination directory if it doesn't exist
    fs::create_dir_all(dst)?;

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        let dst_path = dst.join(path.file_name().unwrap());

        if path.is_dir() {
            // Recursively copy subdirectories
            copy_dir_recursive(&path, &dst_path)?;
        } else {
            // Copy files
            fs::copy(&path, &dst_path)?;
        }
    }
    Ok(())
}

fn prepare_frontend() -> io::Result<PathBuf> {
    // Get the .cmon directory in the home directory
    let home_dir = home::home_dir().ok_or_else(|| io::Error::new(
        io::ErrorKind::NotFound, 
        "Could not find home directory"
    ))?;
    
    let cmon_dir = home_dir.join(".cmon");
    let frontend_dir = cmon_dir.join("frontend");

    // Create .cmon directory if it doesn't exist
    fs::create_dir_all(&cmon_dir)?;

    // Copy frontend files
    let source_frontend = Path::new("frontend");
    copy_dir_recursive(source_frontend, &frontend_dir)?;

    // Run yarn install
    let yarn_install = Command::new("yarn")
        .current_dir(&frontend_dir)
        .arg("install")
        .status()?;

    if !yarn_install.success() {
        return Err(io::Error::new(
            io::ErrorKind::Other, 
            "Failed to run yarn install"
        ));
    }

    // Run yarn build
    let yarn_build = Command::new("yarn")
        .current_dir(&frontend_dir)
        .arg("build")
        .status()?;

    if !yarn_build.success() {
        return Err(io::Error::new(
            io::ErrorKind::Other, 
            "Failed to run yarn build"
        ));
    }

    Ok(frontend_dir.join("dist"))
}

async fn chat_handler(
    data: web::Data<AppState>, 
    req: web::Json<ChatRequest>
) -> impl Responder {
    let mut _chat = data.chat.lock().unwrap();

    // Existing chat handler code remains the same
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
            _chat.messages.push(msg.clone());
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

async fn clear_chat_history(
    data: web::Data<AppState>
) -> impl Responder {
    let mut chat = data.chat.lock().unwrap();
    
    // Clear the messages vector
    chat.messages.clear();
    
    HttpResponse::Ok().json(serde_json::json!({
        "status": "Chat history cleared"
    }))
}

#[get("/{filename:.*}")]
async fn index(req: HttpRequest, frontend_dir: web::Data<PathBuf>) -> impl Responder {
    let path = req.match_info().query("filename").to_string();
    let path_to_check = if path.is_empty() { "index.html".to_string() } else { path };

    let full_path = frontend_dir.join(&path_to_check);

    // Try to read the file
    match fs::read(&full_path) {
        Ok(contents) => {
            // Guess the mime type
            let mime_type = from_path(&path_to_check).first_or_octet_stream();
            
            HttpResponse::Ok()
                .content_type(mime_type.as_ref())
                .body(contents)
        },
        Err(_) => {
            // If file not found, serve index.html for client-side routing
            match fs::read(frontend_dir.join("index.html")) {
                Ok(index_contents) => {
                    HttpResponse::Ok()
                        .content_type("text/html")
                        .body(index_contents)
                },
                Err(_) => {
                    HttpResponse::InternalServerError()
                        .body("index.html not found in build directory")
                }
            }
        }
    }
}

pub async fn start_server(host: String, port: u16) -> std::io::Result<()> {
    // Prepare frontend: copy and build
    let frontend_dist_dir = prepare_frontend()?;

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
            .app_data(web::Data::new(frontend_dist_dir.clone()))
            .route("/chat", web::post().to(chat_handler))
            .route("/history", web::get().to(get_chat_history))
            .route("/clear", web::post().to(clear_chat_history))
            .service(index)
    })
    .bind(format!("{}:{}", host, port))?
    .run()
    .await
}