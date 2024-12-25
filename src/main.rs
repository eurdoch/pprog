mod inference;

use inference::query_anthropic;

#[tokio::main]
async fn main() {
    match query_anthropic("HEllo").await {
        Ok(response) => println!("Got response: {:#?}", response),
        Err(e) => eprintln!("Error: {}", e),
    }
}
