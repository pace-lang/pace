use miette::{miette, Result};
use dialoguer::{Input, Password};
use serde_json::json;

pub fn login() -> Result<()> {
    println!("🔐 Log in to the Pace Registry\n");

    let username: String = Input::new()
        .with_prompt("Username")
        .validate_with(|input: &String| -> std::result::Result<(), String> {
            if input.is_empty() {
                Err("Username cannot be empty".into())
            } else if !input.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
                Err("Username can only contain letters, numbers, hyphens, and underscores".into())
            } else {
                Ok(())
            }
        })
        .interact_text()
        .map_err(|e| miette!("Failed to read username: {}", e))?;

    let email: String = Input::new()
        .with_prompt("Email")
        .validate_with(|input: &String| -> std::result::Result<(), String> {
            if input.is_empty() {
                Err("Email cannot be empty".into())
            } else if !input.contains('@') || !input.contains('.') {
                Err("Please enter a valid email address".into())
            } else {
                Ok(())
            }
        })
        .interact_text()
        .map_err(|e| miette!("Failed to read email: {}", e))?;

    let password = Password::new()
        .with_prompt("Password")
        .with_confirmation("Confirm Password", "Passwords mismatching")
        .interact()
        .map_err(|e| miette!("Failed to read password: {}", e))?;

    println!("\nAuthenticating...");

    let response = ureq::post("http://localhost:3000/api/auth/login")
        .send_json(json!({
            "username": username,
            "email": email,
            "password": password
        }));

    match response {
        Ok(res) => {
            if res.status() != 200 {
                let code = res.status();
                let body: serde_json::Value = res.into_body().read_json().unwrap_or(json!({}));
                let err_msg = body.get("error").and_then(|e| e.as_str()).unwrap_or("Unknown error");
                return Err(miette!("Login failed ({}): {}", code, err_msg));
            }
            let body: serde_json::Value = res.into_body().read_json().map_err(|e| miette!("Failed to parse response: {}", e))?;
            if let Some(token) = body.get("token").and_then(|t| t.as_str()) {
                // Save token to ~/.pace/credentials.toml
                let home_dir = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
                let credentials_path = std::path::Path::new(&home_dir).join(".pace").join("credentials.toml");
                
                if let Some(parent) = credentials_path.parent() {
                    std::fs::create_dir_all(parent).map_err(|e| miette!("Failed to create ~/.pace dir: {}", e))?;
                }
                
                let content = format!("token = \"{}\"\n", token);
                std::fs::write(&credentials_path, content).map_err(|e| miette!("Failed to write credentials: {}", e))?;
                
                println!("✅ Successfully logged in! Credentials saved to ~/.pace/credentials.toml");
                Ok(())
            } else {
                Err(miette!("Invalid response from registry: missing token"))
            }
        }
        Err(e) => Err(miette!("Failed to connect to registry: {}", e)),
    }
}
