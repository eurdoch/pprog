use std::env;
use std::path::Path;
use std::process::Command;
use std::fs;

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let frontend_path = Path::new("frontend");
    let dist_path = frontend_path.join("dist");
    
    // Check if frontend directory exists
    if frontend_path.exists() {
        // Create dist directory in OUT_DIR if it doesn't exist
        fs::create_dir_all(Path::new(&out_dir).join("frontend/dist")).expect("Failed to create dist directory");
        
        // Only build if dist doesn't exist or is empty
        if !dist_path.exists() || fs::read_dir(&dist_path).unwrap().next().is_none() {
            println!("cargo:warning=Building frontend...");
            
            // Install dependencies
            let status = Command::new("yarn")
                .current_dir(frontend_path)
                .arg("install")
                .status()
                .expect("Failed to run yarn install");
            
            if !status.success() {
                panic!("Yarn install failed");
            }
            
            // Build frontend
            let status = Command::new("yarn")
                .current_dir(frontend_path)
                .arg("build")
                .status()
                .expect("Failed to run yarn build");
            
            if !status.success() {
                panic!("Yarn build failed");
            }
        }
    }
    
    // Tell cargo to rerun this script if frontend files change
    println!("cargo:rerun-if-changed=frontend/src/*");
    println!("cargo:rerun-if-changed=frontend/public/*");
    println!("cargo:rerun-if-changed=frontend/index.html");
    println!("cargo:rerun-if-changed=frontend/package.json");
}