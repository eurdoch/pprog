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

fn process_files(dir: &Dir, base_path: &str, static_files: &mut HashMap<String, Vec<u8>>, hbs: &mut Handlebars, template_data: &serde_json::Value) {
    for entry in dir.entries() {
        let relative_path = entry.path().to_string_lossy().replace("\\", "/");
        let full_path = format!("{}/{}", base_path, relative_path);
        println!("Processing path: {}", full_path); // Add debug logging

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
            
            // Print out the paths being added to static_files
            println!("Adding static file: {}", normalized_path);
            
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
    
    // Print out the contents of DIST_DIR
    println!("Entries in DIST_DIR:");
    for entry in DIST_DIR.entries() {
        println!("- {:?}", entry.path());
    }

    process_files(&DIST_DIR, "", &mut static_files, &mut hbs, &template_data);

    println!("Processed static files:");
    for (key, _) in &static_files {
        println!("- {}", key);
    }

    let app_state = web::Data::new(AppState {
        chat: Mutex::new(Chat::new()),
        static_files,
    });

    println!("Starting server on {}:{}", host, port);

    HttpServer::new(move || {
        let cors = Cors::default()
            // TODO use host and port parameter
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
            .route("/chat", web::post().to(chat_handler))
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

    // Debug: print out all available static files
    println!("Available static files:");
    for (key, _) in &app_data.as_ref().static_files {
        println!("- {}", key);
    }

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
