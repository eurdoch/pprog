use actix_web::{web, App, HttpServer, HttpResponse, Responder};
use serde::{Serialize};
use std::sync::Mutex;
use anyhow::Result;

pub struct ServerState {
    // Add any state you want to share across requests
}

#[derive(Debug, Serialize)]
struct ApiResponse {
    status: String,
    message: String,
}

pub async fn start_server(host: String, port: u16) -> Result<()> {
    let state = web::Data::new(Mutex::new(ServerState {}));

    println!("Starting server at http://{}:{}", host, port);
    
    HttpServer::new(move || {
        App::new()
            .app_data(state.clone())
            .service(web::scope("/api")
                .route("/health", web::get().to(health_check)))
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
