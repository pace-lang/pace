#[derive(Debug, Clone, Default)]
pub enum OutputFormat {
    #[default]
    Human,
    Json,
}

#[derive(Debug, Clone)]
pub struct Options {
    pub release_mode: bool,
    pub output_format: OutputFormat,
    pub target_platform: String,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            release_mode: false,
            output_format: OutputFormat::Human,
            target_platform: std::env::var("PACE_TARGET").unwrap_or_else(|_| "native".to_string()),
        }
    }
}
