use miette::{Result, miette};
use serde_json::json;

pub fn login(token: String) -> Result<()> {
    println!("🔐 Authenticating with the Pace Registry...");

    let registry_url = std::env::var("PACE_REGISTRY_URL")
        .unwrap_or_else(|_| "https://registry.pace.dev".to_string());

    // Verify token with registry
    let response = ureq::get(&format!("{}/api/auth/verify", registry_url))
        .header("Authorization", &format!("Bearer {}", token))
        .call();

    match response {
        Ok(res) => {
            if res.status() != 200 {
                let code = res.status();
                let body: serde_json::Value = res.into_body().read_json().unwrap_or(json!({}));
                let err_msg = body
                    .get("error")
                    .and_then(|e| e.as_str())
                    .unwrap_or("Unknown error");
                return Err(miette!("Login failed ({}): {}", code, err_msg));
            }

            // Save token to ~/.pace/credentials.toml
            let home_dir = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
            let credentials_path = std::path::Path::new(&home_dir)
                .join(".pace")
                .join("credentials.toml");

            if let Some(parent) = credentials_path.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| miette!("Failed to create ~/.pace dir: {}", e))?;
            }

            let content = format!("token = \"{}\"\n", token);
            std::fs::write(&credentials_path, content)
                .map_err(|e| miette!("Failed to write credentials: {}", e))?;

            println!("✅ Successfully logged in! Credentials saved to ~/.pace/credentials.toml");
            Ok(())
        }
        Err(e) => {
            // Note: If the registry is down or verification endpoint doesn't exist yet,
            // the user might still want to save it locally for dev. But per plan, we verify first.
            Err(miette!(
                "Failed to connect to registry to verify token: {}",
                e
            ))
        }
    }
}
