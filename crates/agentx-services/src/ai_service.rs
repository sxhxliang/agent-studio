//! AI Service - Provides OpenAI-compatible API integration for code annotation
//!
//! This service enables AI-powered features like:
//! - Code documentation generation
//! - Code explanation
//! - Optimization suggestions

use std::collections::HashMap;
use std::sync::{Arc, OnceLock, RwLock};

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};

use agentx_types::ModelConfig;

/// Global Tokio runtime for HTTP requests
static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();

/// AI service for code annotation and analysis
pub struct AiService {
    /// Shared HTTP client for making API requests
    http_client: reqwest::Client,
    /// Tokio runtime handle for async operations
    runtime_handle: tokio::runtime::Handle,
    /// Service configuration
    pub config: Arc<RwLock<AiServiceConfig>>,
}

/// Configuration for AI service
#[derive(Clone)]
pub struct AiServiceConfig {
    /// Available AI models from config.json
    pub models: HashMap<String, ModelConfig>,
    /// Default model to use (first enabled model)
    pub default_model: Option<String>,
    /// Global system prompts for AI features
    /// Keys: "doc_comment", "inline_comment", "explain", "improve"
    pub system_prompts: HashMap<String, String>,
}

/// Style of code comment to generate
#[derive(Clone, Copy, Debug)]
pub enum CommentStyle {
    /// Multi-line documentation comment (///, /** */, """)
    FunctionDoc,
    /// Single-line inline comment (//, #)
    Inline,
}

/// Request body for OpenAI Chat Completions API
#[derive(Debug, Serialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
}

#[derive(Debug, Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

/// Response from OpenAI Chat Completions API
#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ChatMessageResponse,
}

#[derive(Debug, Deserialize)]
struct ChatMessageResponse {
    content: String,
}

impl AiService {
    /// Create a new AI service with the given model configurations and system prompts
    pub fn new(
        models: HashMap<String, ModelConfig>,
        system_prompts: HashMap<String, String>,
    ) -> Self {
        // Get or create Tokio runtime
        let runtime_handle = tokio::runtime::Handle::try_current().unwrap_or_else(|_| {
            log::debug!("No Tokio runtime found, creating one for AI service...");
            let runtime = RUNTIME.get_or_init(|| {
                tokio::runtime::Builder::new_multi_thread()
                    .worker_threads(2)
                    .enable_all()
                    .build()
                    .expect("Failed to initialize Tokio runtime for AI service")
            });
            runtime.handle().clone()
        });

        // Build HTTP client with timeout
        let http_client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        // Find first enabled model as default
        let default_model = models
            .iter()
            .find(|(_, config)| config.enabled)
            .map(|(name, _)| name.clone());

        if default_model.is_none() {
            log::warn!("No enabled AI models found in configuration");
        } else {
            log::info!("Default AI model: {:?}", default_model);
        }

        let config = AiServiceConfig {
            models,
            default_model,
            system_prompts,
        };

        Self {
            http_client,
            runtime_handle,
            config: Arc::new(RwLock::new(config)),
        }
    }

    /// Update service configuration (for hot-reload support)
    pub fn update_config(
        &self,
        models: HashMap<String, ModelConfig>,
        system_prompts: HashMap<String, String>,
    ) {
        let default_model = models
            .iter()
            .find(|(_, config)| config.enabled)
            .map(|(name, _)| name.clone());

        let mut config = self.config.write().unwrap();
        config.models = models;
        config.default_model = default_model;
        config.system_prompts = system_prompts;

        log::info!("AI Service configuration updated");
    }

    /// Get system prompt from config or use default
    fn get_system_prompt(&self, prompt_key: &str, default_prompt: &str) -> String {
        let config = self.config.read().unwrap();

        if let Some(custom_prompt) = config.system_prompts.get(prompt_key) {
            log::debug!("Using custom system prompt for '{}'", prompt_key);
            return custom_prompt.clone();
        }

        log::debug!("Using default system prompt for '{}'", prompt_key);
        default_prompt.to_string()
    }

    /// Call OpenAI-compatible API with system and user prompts
    async fn call_api(
        &self,
        system_prompt: &str,
        user_prompt: &str,
        max_tokens: Option<u32>,
    ) -> Result<String> {
        // Extract config data and release lock immediately
        let (url, model_name, api_key) = {
            let config = self.config.read().unwrap();

            let model_name = config
                .default_model
                .as_ref()
                .ok_or_else(|| anyhow!("No default AI model configured"))?;

            let model_config = config
                .models
                .get(model_name)
                .ok_or_else(|| anyhow!("Model '{}' not found in configuration", model_name))?;

            if !model_config.enabled {
                return Err(anyhow!("Model '{}' is disabled", model_name));
            }

            let url = format!(
                "{}/chat/completions",
                model_config.base_url.trim_end_matches('/')
            );

            (
                url,
                model_config.model_name.clone(),
                model_config.api_key.clone(),
            )
        }; // Lock is released here

        let request = ChatCompletionRequest {
            model: model_name.clone(),
            messages: vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: system_prompt.to_string(),
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: user_prompt.to_string(),
                },
            ],
            max_tokens,
            temperature: Some(0.3),
        };

        log::debug!("Calling AI API: {} (model: {})", url, model_name);

        let body = serde_json::to_string(&request).context("Failed to serialize request")?;

        // Execute HTTP request in Tokio runtime
        let http_client = self.http_client.clone();

        let response = self
            .runtime_handle
            .spawn(async move {
                http_client
                    .post(&url)
                    .header("Content-Type", "application/json")
                    .header("Authorization", format!("Bearer {}", api_key))
                    .body(body)
                    .send()
                    .await
            })
            .await
            .context("Failed to spawn HTTP request task")?
            .context("Failed to send request to AI service")?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());

            return Err(match status.as_u16() {
                401 => anyhow!("Invalid API key. Please check your config.json"),
                429 => anyhow!("API rate limit reached. Please try again later"),
                500..=599 => anyhow!("AI service error: {}", error_text),
                _ => anyhow!("API request failed ({}): {}", status, error_text),
            });
        }

        let response_text = response
            .text()
            .await
            .context("Failed to read AI service response")?;

        let completion: ChatCompletionResponse =
            serde_json::from_str(&response_text).context("Failed to parse AI service response")?;

        let content = completion
            .choices
            .first()
            .map(|choice| choice.message.content.trim().to_string())
            .ok_or_else(|| anyhow!("No response from AI service"))?;

        Ok(content)
    }

    /// Generate code comment in the specified style
    ///
    /// # Arguments
    /// * `code` - The code to document
    /// * `style` - Comment style (FunctionDoc or Inline)
    ///
    /// # Returns
    /// Raw comment text without formatting (formatting is done by caller)
    pub async fn generate_comment(&self, code: &str, style: CommentStyle) -> Result<String> {
        let (prompt_key, default_system, user_prompt, max_tokens) = match style {
            CommentStyle::FunctionDoc => (
                "doc_comment",
                "You are a code documentation expert. Generate clear, concise documentation comments for code. \
                 Focus on what the code does, parameters, return values, and any important notes. \
                 Return ONLY the comment text without any formatting markers (no ///, /**, etc.).",
                format!("Generate documentation for the following code:\n\n{}", code),
                Some(500),
            ),
            CommentStyle::Inline => (
                "inline_comment",
                "You are a code documentation expert. Generate brief, single-line inline comments that explain code. \
                 Be concise and clear. Return ONLY the comment text without any formatting markers (no //, #, etc.).",
                format!("Generate a brief inline comment for:\n\n{}", code),
                Some(100),
            ),
        };

        let system_prompt = self.get_system_prompt(prompt_key, default_system);

        self.call_api(&system_prompt, &user_prompt, max_tokens)
            .await
            .context("Failed to generate code comment")
    }

    /// Explain what a piece of code does
    ///
    /// # Arguments
    /// * `code` - The code to explain
    ///
    /// # Returns
    /// Natural language explanation of the code
    pub async fn explain_code(&self, code: &str) -> Result<String> {
        let default_system = "You are a code explanation expert. Explain code clearly and concisely \
                            in natural language. Focus on what the code does, why it works that way, \
                            and any important concepts.";

        let system_prompt = self.get_system_prompt("explain", default_system);
        let user_prompt = format!("Explain what this code does:\n\n{}", code);

        self.call_api(&system_prompt, &user_prompt, Some(500))
            .await
            .context("Failed to explain code")
    }

    /// Suggest improvements for code
    ///
    /// # Arguments
    /// * `code` - The code to analyze
    ///
    /// # Returns
    /// List of improvement suggestions as numbered list
    pub async fn suggest_improvements(&self, code: &str) -> Result<String> {
        let default_system = "You are a code review expert. Analyze code and suggest improvements \
                            focusing on: readability, performance, best practices, potential bugs, \
                            and maintainability. Format your response as a numbered list.";

        let system_prompt = self.get_system_prompt("improve", default_system);
        let user_prompt = format!(
            "Suggest improvements for this code:\n\n{}\n\nFormat as numbered list.",
            code
        );

        self.call_api(&system_prompt, &user_prompt, Some(800))
            .await
            .context("Failed to generate improvement suggestions")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_config() -> HashMap<String, ModelConfig> {
        let mut models = HashMap::new();
        models.insert(
            "test-model".to_string(),
            ModelConfig {
                enabled: true,
                provider: "openai".to_string(),
                base_url: "https://api.openai.com/v1".to_string(),
                api_key: "test-key".to_string(),
                model_name: "gpt-3.5-turbo".to_string(),
            },
        );
        models
    }

    #[test]
    fn test_ai_service_creation() {
        let models = create_test_config();
        let service = AiService::new(models, HashMap::new());

        let config = service.config.read().unwrap();
        assert!(config.default_model.is_some());
        assert_eq!(config.default_model.as_ref().unwrap(), "test-model");
    }

    #[test]
    fn test_config_update() {
        let models = create_test_config();
        let service = AiService::new(models, HashMap::new());

        let mut new_models = HashMap::new();
        new_models.insert(
            "new-model".to_string(),
            ModelConfig {
                enabled: true,
                provider: "openai".to_string(),
                base_url: "https://api.openai.com/v1".to_string(),
                api_key: "new-key".to_string(),
                model_name: "gpt-4".to_string(),
            },
        );

        service.update_config(new_models, HashMap::new());

        let config = service.config.read().unwrap();
        assert_eq!(config.default_model.as_ref().unwrap(), "new-model");
    }

    #[test]
    fn test_no_enabled_models() {
        let mut models = HashMap::new();
        models.insert(
            "disabled-model".to_string(),
            ModelConfig {
                enabled: false,
                provider: "openai".to_string(),
                base_url: "https://api.openai.com/v1".to_string(),
                api_key: "test-key".to_string(),
                model_name: "gpt-3.5-turbo".to_string(),
            },
        );

        let service = AiService::new(models, HashMap::new());
        let config = service.config.read().unwrap();
        assert!(config.default_model.is_none());
    }
}
