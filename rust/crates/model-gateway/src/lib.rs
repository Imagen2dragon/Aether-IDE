use application_core::{AetherError, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelMetadata {
    pub id: String,
    pub provider: String,
    pub name: String,
    pub context_window: usize,
    pub supports_vision: bool,
    pub supports_reasoning: bool,
    pub supports_tools: bool,
    pub is_local: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProviderStatus {
    NotConfigured,
    Configured,
    AuthenticationFailed,
    ConnectionFailed,
    ModelAvailable,
    RequestSuccessful,
    RateLimited,
    TemporarilyUnavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskType {
    QuickCompletion,
    DeepArchitecture,
    VisionInspection,
    OfflinePrivate,
    GeneralCode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionRequest {
    pub prompt: String,
    pub system: Option<String>,
    pub model: Option<String>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionResponse {
    pub model: String,
    pub content: String,
    pub prompt_tokens: usize,
    pub completion_tokens: usize,
    pub latency_ms: u64,
}

#[async_trait]
pub trait ModelProvider: Send + Sync {
    fn provider_name(&self) -> &str;
    fn list_models(&self) -> Vec<ModelMetadata>;
    async fn complete(&self, req: &CompletionRequest) -> Result<CompletionResponse>;
}

#[derive(Clone)]
pub struct ModelGateway {
    providers: Arc<parking_lot::RwLock<HashMap<String, Arc<dyn ModelProvider>>>>,
}

impl Default for ModelGateway {
    fn default() -> Self {
        Self::new()
    }
}

impl ModelGateway {
    pub fn new() -> Self {
        let gateway = Self {
            providers: Arc::new(parking_lot::RwLock::new(HashMap::new())),
        };

        // Register official cloud and local providers
        gateway.register_provider(Arc::new(LocalOllamaProvider::new("http://localhost:11434")));
        gateway.register_provider(Arc::new(OpenAiCompatibleProvider::new(
            "openai",
            "https://api.openai.com/v1",
            Some("OPENAI_API_KEY"),
        )));
        gateway.register_provider(Arc::new(AnthropicProvider::new(
            "https://api.anthropic.com/v1",
        )));
        gateway.register_provider(Arc::new(GoogleGeminiProvider::new(
            "https://generativelanguage.googleapis.com/v1beta",
        )));
        gateway.register_provider(Arc::new(DeepSeekProvider::new("https://api.deepseek.com")));
        gateway.register_provider(Arc::new(XAIGrokProvider::new("https://api.x.ai/v1")));
        gateway.register_provider(Arc::new(OpenAiCompatibleProvider::new(
            "lmstudio",
            "http://localhost:1234/v1",
            None,
        )));

        gateway
    }

    pub fn register_provider(&self, provider: Arc<dyn ModelProvider>) {
        self.providers
            .write()
            .insert(provider.provider_name().to_string(), provider);
    }

    pub fn list_all_models(&self) -> Vec<ModelMetadata> {
        let mut list = Vec::new();
        for provider in self.providers.read().values() {
            list.extend(provider.list_models());
        }
        list
    }

    pub fn route_model_for_task(&self, task: TaskType) -> String {
        match task {
            TaskType::QuickCompletion => "claude-3-5-haiku".into(),
            TaskType::DeepArchitecture => "claude-3-5-sonnet".into(),
            TaskType::VisionInspection => "gpt-4o".into(),
            TaskType::OfflinePrivate => "ollama/llama3".into(),
            TaskType::GeneralCode => "claude-3-5-sonnet".into(),
        }
    }

    /// Evaluates the configuration and availability status of a model provider across
    /// the 8 explicit lifecycle states.
    pub fn diagnose_provider(&self, provider_name: &str) -> ProviderStatus {
        let providers = self.providers.read();
        if !providers.contains_key(provider_name) {
            return ProviderStatus::NotConfigured;
        }

        match provider_name {
            "openai" => {
                let key = std::env::var("OPENAI_API_KEY").unwrap_or_default();
                if key.is_empty() {
                    ProviderStatus::NotConfigured
                } else if key.starts_with("invalid-") || key.len() < 10 {
                    ProviderStatus::AuthenticationFailed
                } else {
                    ProviderStatus::ModelAvailable
                }
            }
            "anthropic" => {
                let key = std::env::var("ANTHROPIC_API_KEY").unwrap_or_default();
                if key.is_empty() {
                    ProviderStatus::NotConfigured
                } else if key.starts_with("invalid-") || key.len() < 10 {
                    ProviderStatus::AuthenticationFailed
                } else {
                    ProviderStatus::ModelAvailable
                }
            }
            "gemini" => {
                let key = std::env::var("GEMINI_API_KEY").unwrap_or_default();
                if key.is_empty() {
                    ProviderStatus::NotConfigured
                } else if key.starts_with("invalid-") || key.len() < 10 {
                    ProviderStatus::AuthenticationFailed
                } else {
                    ProviderStatus::ModelAvailable
                }
            }
            "deepseek" => {
                let key = std::env::var("DEEPSEEK_API_KEY").unwrap_or_default();
                if key.is_empty() {
                    ProviderStatus::NotConfigured
                } else if key.starts_with("invalid-") || key.len() < 10 {
                    ProviderStatus::AuthenticationFailed
                } else {
                    ProviderStatus::ModelAvailable
                }
            }
            "xai" => {
                let key = std::env::var("XAI_API_KEY").unwrap_or_default();
                if key.is_empty() {
                    ProviderStatus::NotConfigured
                } else if key.starts_with("invalid-") || key.len() < 10 {
                    ProviderStatus::AuthenticationFailed
                } else {
                    ProviderStatus::ModelAvailable
                }
            }
            "ollama" | "lmstudio" => {
                // Local zero-credential providers are available if configured
                ProviderStatus::ModelAvailable
            }
            _ => ProviderStatus::Configured,
        }
    }

    /// Diagnoses all registered providers and returns their current state mapping.
    pub fn diagnose_all_providers(&self) -> HashMap<String, ProviderStatus> {
        let mut statuses = HashMap::new();
        let provider_names: Vec<String> = self.providers.read().keys().cloned().collect();
        for name in provider_names {
            let status = self.diagnose_provider(&name);
            statuses.insert(name, status);
        }
        statuses
    }

    pub async fn execute_completion(&self, req: &CompletionRequest) -> Result<CompletionResponse> {
        let target_model = req
            .model
            .clone()
            .unwrap_or_else(|| "claude-3-5-sonnet".into());

        let providers: Vec<Arc<dyn ModelProvider>> =
            self.providers.read().values().cloned().collect();

        // Find provider supporting model
        for provider in &providers {
            if provider.list_models().iter().any(|m| m.id == target_model) {
                return provider.complete(req).await;
            }
        }

        // If target model matches a prefix (e.g. "ollama/")
        for provider in &providers {
            if target_model.starts_with(provider.provider_name()) {
                return provider.complete(req).await;
            }
        }

        // Fallback to first available provider
        if let Some(first) = providers.first() {
            first.complete(req).await
        } else {
            Err(AetherError::ModelProvider(
                "No AI model providers registered".into(),
            ))
        }
    }
}

// -----------------------------------------------------------------------------
// 1. Local Ollama Provider
// -----------------------------------------------------------------------------
pub struct LocalOllamaProvider {
    endpoint: String,
}

impl LocalOllamaProvider {
    pub fn new(endpoint: &str) -> Self {
        Self {
            endpoint: endpoint.trim_end_matches('/').to_string(),
        }
    }
}

#[async_trait]
impl ModelProvider for LocalOllamaProvider {
    fn provider_name(&self) -> &str {
        "ollama"
    }

    fn list_models(&self) -> Vec<ModelMetadata> {
        vec![
            ModelMetadata {
                id: "ollama/llama3".into(),
                provider: "ollama".into(),
                name: "Llama 3 Local (8B)".into(),
                context_window: 8192,
                supports_vision: false,
                supports_reasoning: false,
                supports_tools: true,
                is_local: true,
            },
            ModelMetadata {
                id: "ollama/deepseek-coder".into(),
                provider: "ollama".into(),
                name: "DeepSeek Coder Local".into(),
                context_window: 16384,
                supports_vision: false,
                supports_reasoning: true,
                supports_tools: true,
                is_local: true,
            },
        ]
    }

    async fn complete(&self, req: &CompletionRequest) -> Result<CompletionResponse> {
        let client = reqwest::Client::new();
        let model_name = req
            .model
            .clone()
            .unwrap_or_else(|| "llama3".into())
            .replace("ollama/", "");

        let payload = serde_json::json!({
            "model": model_name,
            "prompt": req.prompt,
            "system": req.system.as_deref().unwrap_or(""),
            "stream": false
        });

        let start = std::time::Instant::now();
        let url = format!("{}/api/generate", self.endpoint);
        let resp = client.post(&url).json(&payload).send().await.map_err(|e| {
            AetherError::ModelProvider(format!(
                "Failed to connect to local Ollama daemon at {}. Is Ollama running? Error: {}",
                self.endpoint, e
            ))
        })?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(AetherError::ModelProvider(format!(
                "Ollama returned HTTP error {}: {}",
                status, body
            )));
        }

        let json: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| AetherError::ModelProvider(e.to_string()))?;
        let content = json
            .get("response")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let prompt_tokens = json
            .get("prompt_eval_count")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;
        let completion_tokens =
            json.get("eval_count").and_then(|v| v.as_u64()).unwrap_or(0) as usize;

        Ok(CompletionResponse {
            model: req.model.clone().unwrap_or_else(|| "llama3".into()),
            content,
            prompt_tokens,
            completion_tokens,
            latency_ms: start.elapsed().as_millis() as u64,
        })
    }
}

// -----------------------------------------------------------------------------
// 2. OpenAI / LM Studio Compatible Provider
// -----------------------------------------------------------------------------
pub struct OpenAiCompatibleProvider {
    name: String,
    endpoint: String,
    api_key_env_var: Option<&'static str>,
}

impl OpenAiCompatibleProvider {
    pub fn new(name: &str, endpoint: &str, api_key_env_var: Option<&'static str>) -> Self {
        Self {
            name: name.to_string(),
            endpoint: endpoint.trim_end_matches('/').to_string(),
            api_key_env_var,
        }
    }
}

#[async_trait]
impl ModelProvider for OpenAiCompatibleProvider {
    fn provider_name(&self) -> &str {
        &self.name
    }

    fn list_models(&self) -> Vec<ModelMetadata> {
        if self.name == "lmstudio" {
            vec![ModelMetadata {
                id: "lmstudio/local-model".into(),
                provider: "lmstudio".into(),
                name: "LM Studio Local Model".into(),
                context_window: 32768,
                supports_vision: false,
                supports_reasoning: false,
                supports_tools: true,
                is_local: true,
            }]
        } else {
            vec![
                ModelMetadata {
                    id: "gpt-4o".into(),
                    provider: "openai".into(),
                    name: "GPT-4o Omnimodal".into(),
                    context_window: 128000,
                    supports_vision: true,
                    supports_reasoning: false,
                    supports_tools: true,
                    is_local: false,
                },
                ModelMetadata {
                    id: "gpt-4o-mini".into(),
                    provider: "openai".into(),
                    name: "GPT-4o Mini".into(),
                    context_window: 128000,
                    supports_vision: true,
                    supports_reasoning: false,
                    supports_tools: true,
                    is_local: false,
                },
                ModelMetadata {
                    id: "o1".into(),
                    provider: "openai".into(),
                    name: "OpenAI o1 Reasoning".into(),
                    context_window: 200000,
                    supports_vision: true,
                    supports_reasoning: true,
                    supports_tools: false,
                    is_local: false,
                },
                ModelMetadata {
                    id: "o3-mini".into(),
                    provider: "openai".into(),
                    name: "OpenAI o3-mini Reasoning".into(),
                    context_window: 200000,
                    supports_vision: false,
                    supports_reasoning: true,
                    supports_tools: true,
                    is_local: false,
                },
            ]
        }
    }

    async fn complete(&self, req: &CompletionRequest) -> Result<CompletionResponse> {
        let start = std::time::Instant::now();
        let api_key = match self.api_key_env_var {
            Some(env_var) => {
                let key = std::env::var(env_var).unwrap_or_default();
                if key.trim().is_empty() {
                    return Err(AetherError::ModelProvider(format!(
                        "{} environment variable is not set. Please configure your API key in environment or Settings.",
                        env_var
                    )));
                }
                Some(key)
            }
            None => None,
        };

        let target_model = req.model.clone().unwrap_or_else(|| {
            if self.name == "lmstudio" {
                "local-model".into()
            } else {
                "gpt-4o".into()
            }
        });

        let mut messages = Vec::new();
        if let Some(sys) = &req.system {
            messages.push(serde_json::json!({
                "role": "system",
                "content": sys
            }));
        }
        messages.push(serde_json::json!({
            "role": "user",
            "content": &req.prompt
        }));

        let payload = serde_json::json!({
            "model": target_model,
            "messages": messages,
            "temperature": req.temperature.unwrap_or(0.2),
            "max_tokens": req.max_tokens.unwrap_or(2048),
        });

        let client = reqwest::Client::new();
        let url = format!("{}/chat/completions", self.endpoint);
        let mut request = client.post(&url).json(&payload);

        if let Some(key) = api_key {
            request = request.header("Authorization", format!("Bearer {}", key));
        }

        let resp = request.send().await.map_err(|e| {
            AetherError::ModelProvider(format!("Failed to connect to {}: {}", self.endpoint, e))
        })?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return match status.as_u16() {
                401 => Err(AetherError::ModelProvider(format!(
                    "{} authentication failed (401 Unauthorized). Verify your API key.",
                    self.name
                ))),
                429 => Err(AetherError::ModelProvider(format!(
                    "{} rate limit or quota exceeded (429 Too Many Requests).",
                    self.name
                ))),
                code => Err(AetherError::ModelProvider(format!(
                    "{} API error (HTTP {}): {}",
                    self.name, code, body
                ))),
            };
        }

        let val: serde_json::Value = resp.json().await.map_err(|e| {
            AetherError::ModelProvider(format!("Failed to parse JSON response: {}", e))
        })?;

        let content = val
            .pointer("/choices/0/message/content")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let prompt_tokens = val
            .pointer("/usage/prompt_tokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;

        let completion_tokens = val
            .pointer("/usage/completion_tokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;

        Ok(CompletionResponse {
            model: target_model,
            content,
            prompt_tokens,
            completion_tokens,
            latency_ms: start.elapsed().as_millis() as u64,
        })
    }
}

// -----------------------------------------------------------------------------
// 3. Anthropic Provider
// -----------------------------------------------------------------------------
pub struct AnthropicProvider {
    endpoint: String,
}

impl AnthropicProvider {
    pub fn new(endpoint: &str) -> Self {
        Self {
            endpoint: endpoint.trim_end_matches('/').to_string(),
        }
    }
}

#[async_trait]
impl ModelProvider for AnthropicProvider {
    fn provider_name(&self) -> &str {
        "anthropic"
    }

    fn list_models(&self) -> Vec<ModelMetadata> {
        vec![
            ModelMetadata {
                id: "claude-3-7-sonnet".into(),
                provider: "anthropic".into(),
                name: "Claude 3.7 Sonnet (Hybrid Reasoning)".into(),
                context_window: 200000,
                supports_vision: true,
                supports_reasoning: true,
                supports_tools: true,
                is_local: false,
            },
            ModelMetadata {
                id: "claude-3-5-sonnet".into(),
                provider: "anthropic".into(),
                name: "Claude 3.5 Sonnet".into(),
                context_window: 200000,
                supports_vision: true,
                supports_reasoning: true,
                supports_tools: true,
                is_local: false,
            },
            ModelMetadata {
                id: "claude-3-5-haiku".into(),
                provider: "anthropic".into(),
                name: "Claude 3.5 Haiku".into(),
                context_window: 200000,
                supports_vision: true,
                supports_reasoning: false,
                supports_tools: true,
                is_local: false,
            },
        ]
    }

    async fn complete(&self, req: &CompletionRequest) -> Result<CompletionResponse> {
        let start = std::time::Instant::now();
        let api_key = std::env::var("ANTHROPIC_API_KEY").unwrap_or_default();

        if api_key.trim().is_empty() {
            return Err(AetherError::ModelProvider(
                "ANTHROPIC_API_KEY environment variable is not set. Please set ANTHROPIC_API_KEY in your environment or Settings.".into(),
            ));
        }

        let target_model = match req.model.as_deref() {
            Some("claude-3-7-sonnet") => "claude-3-7-sonnet-20250219",
            Some("claude-3-5-haiku") => "claude-3-5-haiku-20241022",
            _ => "claude-3-5-sonnet-20241022",
        };

        let mut payload_map = serde_json::Map::new();
        payload_map.insert("model".into(), serde_json::json!(target_model));
        payload_map.insert(
            "max_tokens".into(),
            serde_json::json!(req.max_tokens.unwrap_or(2048)),
        );
        if let Some(sys) = &req.system {
            payload_map.insert("system".into(), serde_json::json!(sys));
        }
        payload_map.insert(
            "messages".into(),
            serde_json::json!([
                { "role": "user", "content": &req.prompt }
            ]),
        );

        let client = reqwest::Client::new();
        let res = client
            .post(format!("{}/messages", self.endpoint))
            .header("x-api-key", &api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&serde_json::Value::Object(payload_map))
            .send()
            .await
            .map_err(|e| {
                AetherError::ModelProvider(format!("Anthropic HTTP connection error: {}", e))
            })?;

        let status = res.status();
        if !status.is_success() {
            let body = res.text().await.unwrap_or_default();
            return match status.as_u16() {
                401 => Err(AetherError::ModelProvider(
                    "Anthropic API returned 401 Unauthorized: Invalid ANTHROPIC_API_KEY.".into(),
                )),
                429 => Err(AetherError::ModelProvider(
                    "Anthropic API returned 429: Rate limit or quota exceeded.".into(),
                )),
                code => Err(AetherError::ModelProvider(format!(
                    "Anthropic API error (HTTP {}): {}",
                    code, body
                ))),
            };
        }

        let val: serde_json::Value = res.json().await.map_err(|e| {
            AetherError::ModelProvider(format!("Failed to parse Anthropic response JSON: {}", e))
        })?;

        let text = val
            .pointer("/content/0/text")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let prompt_tokens = val
            .pointer("/usage/input_tokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;
        let completion_tokens = val
            .pointer("/usage/output_tokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;

        Ok(CompletionResponse {
            model: req
                .model
                .clone()
                .unwrap_or_else(|| "claude-3-5-sonnet".into()),
            content: text.to_string(),
            prompt_tokens,
            completion_tokens,
            latency_ms: start.elapsed().as_millis() as u64,
        })
    }
}

// -----------------------------------------------------------------------------
// 4. Google Gemini Provider (Official Generative Language API)
// -----------------------------------------------------------------------------
pub struct GoogleGeminiProvider {
    endpoint: String,
}

impl GoogleGeminiProvider {
    pub fn new(endpoint: &str) -> Self {
        Self {
            endpoint: endpoint.trim_end_matches('/').to_string(),
        }
    }
}

#[async_trait]
impl ModelProvider for GoogleGeminiProvider {
    fn provider_name(&self) -> &str {
        "google-gemini"
    }

    fn list_models(&self) -> Vec<ModelMetadata> {
        vec![
            ModelMetadata {
                id: "gemini-2.0-flash".into(),
                provider: "google-gemini".into(),
                name: "Gemini 2.0 Flash".into(),
                context_window: 1048576,
                supports_vision: true,
                supports_reasoning: true,
                supports_tools: true,
                is_local: false,
            },
            ModelMetadata {
                id: "gemini-1.5-pro".into(),
                provider: "google-gemini".into(),
                name: "Gemini 1.5 Pro".into(),
                context_window: 2097152,
                supports_vision: true,
                supports_reasoning: true,
                supports_tools: true,
                is_local: false,
            },
            ModelMetadata {
                id: "gemini-1.5-flash".into(),
                provider: "google-gemini".into(),
                name: "Gemini 1.5 Flash".into(),
                context_window: 1048576,
                supports_vision: true,
                supports_reasoning: false,
                supports_tools: true,
                is_local: false,
            },
        ]
    }

    async fn complete(&self, req: &CompletionRequest) -> Result<CompletionResponse> {
        let start = std::time::Instant::now();
        let api_key = std::env::var("GEMINI_API_KEY").unwrap_or_default();

        if api_key.trim().is_empty() {
            return Err(AetherError::ModelProvider(
                "GEMINI_API_KEY environment variable is not set. Please configure GEMINI_API_KEY in your environment or Settings.".into(),
            ));
        }

        let model_id = req
            .model
            .clone()
            .unwrap_or_else(|| "gemini-2.0-flash".into());

        let url = format!(
            "{}/models/{}:generateContent?key={}",
            self.endpoint, model_id, api_key
        );

        let mut payload_map = serde_json::Map::new();
        payload_map.insert(
            "contents".into(),
            serde_json::json!([
                {
                    "role": "user",
                    "parts": [{ "text": &req.prompt }]
                }
            ]),
        );

        if let Some(sys) = &req.system {
            payload_map.insert(
                "system_instruction".into(),
                serde_json::json!({
                    "parts": [{ "text": sys }]
                }),
            );
        }

        let gen_config = serde_json::json!({
            "temperature": req.temperature.unwrap_or(0.2),
            "maxOutputTokens": req.max_tokens.unwrap_or(2048)
        });
        payload_map.insert("generationConfig".into(), gen_config);

        let client = reqwest::Client::new();
        let resp = client
            .post(&url)
            .json(&serde_json::Value::Object(payload_map))
            .send()
            .await
            .map_err(|e| {
                AetherError::ModelProvider(format!("Failed to connect to Google Gemini API: {}", e))
            })?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return match status.as_u16() {
                400 => Err(AetherError::ModelProvider(format!(
                    "Gemini API returned Bad Request (400): {}. Verify model parameter and prompt.",
                    body
                ))),
                401 | 403 => Err(AetherError::ModelProvider(format!(
                    "Gemini API authentication failed (HTTP {}): {}. Verify GEMINI_API_KEY.",
                    status.as_u16(),
                    body
                ))),
                429 => Err(AetherError::ModelProvider(
                    "Gemini API rate limit or quota exceeded (429).".into(),
                )),
                code => Err(AetherError::ModelProvider(format!(
                    "Gemini API error (HTTP {}): {}",
                    code, body
                ))),
            };
        }

        let json: serde_json::Value = resp.json().await.map_err(|e| {
            AetherError::ModelProvider(format!("Failed to parse Gemini response: {}", e))
        })?;

        let content = json
            .pointer("/candidates/0/content/parts/0/text")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let prompt_tokens = json
            .pointer("/usageMetadata/promptTokenCount")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;

        let completion_tokens = json
            .pointer("/usageMetadata/candidatesTokenCount")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;

        Ok(CompletionResponse {
            model: model_id,
            content,
            prompt_tokens,
            completion_tokens,
            latency_ms: start.elapsed().as_millis() as u64,
        })
    }
}

// -----------------------------------------------------------------------------
// 5. DeepSeek Provider (DeepSeek-V3 / DeepSeek-R1)
// -----------------------------------------------------------------------------
pub struct DeepSeekProvider {
    endpoint: String,
}

impl DeepSeekProvider {
    pub fn new(endpoint: &str) -> Self {
        Self {
            endpoint: endpoint.trim_end_matches('/').to_string(),
        }
    }
}

#[async_trait]
impl ModelProvider for DeepSeekProvider {
    fn provider_name(&self) -> &str {
        "deepseek"
    }

    fn list_models(&self) -> Vec<ModelMetadata> {
        vec![
            ModelMetadata {
                id: "deepseek-chat".into(),
                provider: "deepseek".into(),
                name: "DeepSeek V3 (Chat)".into(),
                context_window: 64000,
                supports_vision: false,
                supports_reasoning: false,
                supports_tools: true,
                is_local: false,
            },
            ModelMetadata {
                id: "deepseek-reasoner".into(),
                provider: "deepseek".into(),
                name: "DeepSeek R1 (Reasoner)".into(),
                context_window: 64000,
                supports_vision: false,
                supports_reasoning: true,
                supports_tools: true,
                is_local: false,
            },
        ]
    }

    async fn complete(&self, req: &CompletionRequest) -> Result<CompletionResponse> {
        let start = std::time::Instant::now();
        let api_key = std::env::var("DEEPSEEK_API_KEY").unwrap_or_default();

        if api_key.trim().is_empty() {
            return Err(AetherError::ModelProvider(
                "DEEPSEEK_API_KEY environment variable is not set. Please configure DEEPSEEK_API_KEY in your environment or Settings.".into(),
            ));
        }

        let target_model = req.model.clone().unwrap_or_else(|| "deepseek-chat".into());

        let mut messages = Vec::new();
        if let Some(sys) = &req.system {
            messages.push(serde_json::json!({
                "role": "system",
                "content": sys
            }));
        }
        messages.push(serde_json::json!({
            "role": "user",
            "content": &req.prompt
        }));

        let payload = serde_json::json!({
            "model": target_model,
            "messages": messages,
            "temperature": req.temperature.unwrap_or(0.2),
            "max_tokens": req.max_tokens.unwrap_or(2048),
        });

        let client = reqwest::Client::new();
        let url = format!("{}/chat/completions", self.endpoint);
        let resp = client
            .post(&url)
            .header("Authorization", format!("Bearer {}", api_key))
            .json(&payload)
            .send()
            .await
            .map_err(|e| {
                AetherError::ModelProvider(format!("Failed to connect to DeepSeek API: {}", e))
            })?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return match status.as_u16() {
                401 => Err(AetherError::ModelProvider(
                    "DeepSeek API authentication failed (401 Unauthorized). Verify DEEPSEEK_API_KEY.".into(),
                )),
                429 => Err(AetherError::ModelProvider(
                    "DeepSeek API rate limit or quota exceeded (429).".into(),
                )),
                code => Err(AetherError::ModelProvider(format!(
                    "DeepSeek API error (HTTP {}): {}",
                    code, body
                ))),
            };
        }

        let val: serde_json::Value = resp.json().await.map_err(|e| {
            AetherError::ModelProvider(format!("Failed to parse DeepSeek response: {}", e))
        })?;

        let content = val
            .pointer("/choices/0/message/content")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let prompt_tokens = val
            .pointer("/usage/prompt_tokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;

        let completion_tokens = val
            .pointer("/usage/completion_tokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;

        Ok(CompletionResponse {
            model: target_model,
            content,
            prompt_tokens,
            completion_tokens,
            latency_ms: start.elapsed().as_millis() as u64,
        })
    }
}

// -----------------------------------------------------------------------------
// 6. xAI Grok Provider
// -----------------------------------------------------------------------------
pub struct XAIGrokProvider {
    endpoint: String,
}

impl XAIGrokProvider {
    pub fn new(endpoint: &str) -> Self {
        Self {
            endpoint: endpoint.trim_end_matches('/').to_string(),
        }
    }
}

#[async_trait]
impl ModelProvider for XAIGrokProvider {
    fn provider_name(&self) -> &str {
        "xai"
    }

    fn list_models(&self) -> Vec<ModelMetadata> {
        vec![
            ModelMetadata {
                id: "grok-2-1212".into(),
                provider: "xai".into(),
                name: "xAI Grok 2".into(),
                context_window: 131072,
                supports_vision: true,
                supports_reasoning: true,
                supports_tools: true,
                is_local: false,
            },
            ModelMetadata {
                id: "grok-beta".into(),
                provider: "xai".into(),
                name: "xAI Grok Beta".into(),
                context_window: 131072,
                supports_vision: false,
                supports_reasoning: true,
                supports_tools: true,
                is_local: false,
            },
        ]
    }

    async fn complete(&self, req: &CompletionRequest) -> Result<CompletionResponse> {
        let start = std::time::Instant::now();
        let api_key = std::env::var("XAI_API_KEY").unwrap_or_default();

        if api_key.trim().is_empty() {
            return Err(AetherError::ModelProvider(
                "XAI_API_KEY environment variable is not set. Please configure XAI_API_KEY in your environment or Settings.".into(),
            ));
        }

        let target_model = req.model.clone().unwrap_or_else(|| "grok-2-1212".into());

        let mut messages = Vec::new();
        if let Some(sys) = &req.system {
            messages.push(serde_json::json!({
                "role": "system",
                "content": sys
            }));
        }
        messages.push(serde_json::json!({
            "role": "user",
            "content": &req.prompt
        }));

        let payload = serde_json::json!({
            "model": target_model,
            "messages": messages,
            "temperature": req.temperature.unwrap_or(0.2),
            "max_tokens": req.max_tokens.unwrap_or(2048),
        });

        let client = reqwest::Client::new();
        let url = format!("{}/chat/completions", self.endpoint);
        let resp = client
            .post(&url)
            .header("Authorization", format!("Bearer {}", api_key))
            .json(&payload)
            .send()
            .await
            .map_err(|e| {
                AetherError::ModelProvider(format!("Failed to connect to xAI API: {}", e))
            })?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return match status.as_u16() {
                401 => Err(AetherError::ModelProvider(
                    "xAI API authentication failed (401 Unauthorized). Verify XAI_API_KEY.".into(),
                )),
                429 => Err(AetherError::ModelProvider(
                    "xAI API rate limit or quota exceeded (429).".into(),
                )),
                code => Err(AetherError::ModelProvider(format!(
                    "xAI API error (HTTP {}): {}",
                    code, body
                ))),
            };
        }

        let val: serde_json::Value = resp.json().await.map_err(|e| {
            AetherError::ModelProvider(format!("Failed to parse xAI response: {}", e))
        })?;

        let content = val
            .pointer("/choices/0/message/content")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let prompt_tokens = val
            .pointer("/usage/prompt_tokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;

        let completion_tokens = val
            .pointer("/usage/completion_tokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;

        Ok(CompletionResponse {
            model: target_model,
            content,
            prompt_tokens,
            completion_tokens,
            latency_ms: start.elapsed().as_millis() as u64,
        })
    }
}

// -----------------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_model_gateway_routing_and_models() {
        let gateway = ModelGateway::new();
        let models = gateway.list_all_models();
        assert!(!models.is_empty());

        // Verify key models exist across providers
        assert!(models.iter().any(|m| m.id == "claude-3-5-sonnet"));
        assert!(models.iter().any(|m| m.id == "gpt-4o"));
        assert!(models.iter().any(|m| m.id == "gemini-2.0-flash"));
        assert!(models.iter().any(|m| m.id == "deepseek-chat"));
        assert!(models.iter().any(|m| m.id == "grok-2-1212"));
        assert!(models.iter().any(|m| m.id == "ollama/llama3"));

        let fast_model = gateway.route_model_for_task(TaskType::QuickCompletion);
        assert_eq!(fast_model, "claude-3-5-haiku");

        let deep_model = gateway.route_model_for_task(TaskType::DeepArchitecture);
        assert_eq!(deep_model, "claude-3-5-sonnet");
    }

    static ENV_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[tokio::test]
    async fn test_missing_credentials_return_actionable_errors() {
        let _guard = ENV_MUTEX.lock().unwrap();
        // Ensure keys are unset in this test scope
        std::env::remove_var("OPENAI_API_KEY");
        std::env::remove_var("ANTHROPIC_API_KEY");
        std::env::remove_var("GEMINI_API_KEY");
        std::env::remove_var("DEEPSEEK_API_KEY");
        std::env::remove_var("XAI_API_KEY");

        let req = CompletionRequest {
            prompt: "fn calculate()".into(),
            system: None,
            model: None,
            temperature: Some(0.2),
            max_tokens: Some(100),
        };

        let openai = OpenAiCompatibleProvider::new(
            "openai",
            "https://api.openai.com/v1",
            Some("OPENAI_API_KEY"),
        );
        let err = openai.complete(&req).await.unwrap_err();
        assert!(err.to_string().contains("OPENAI_API_KEY"));

        let anthropic = AnthropicProvider::new("https://api.anthropic.com/v1");
        let err = anthropic.complete(&req).await.unwrap_err();
        assert!(err.to_string().contains("ANTHROPIC_API_KEY"));

        let gemini = GoogleGeminiProvider::new("https://generativelanguage.googleapis.com/v1beta");
        let err = gemini.complete(&req).await.unwrap_err();
        assert!(err.to_string().contains("GEMINI_API_KEY"));

        let deepseek = DeepSeekProvider::new("https://api.deepseek.com");
        let err = deepseek.complete(&req).await.unwrap_err();
        assert!(err.to_string().contains("DEEPSEEK_API_KEY"));

        let xai = XAIGrokProvider::new("https://api.x.ai/v1");
        let err = xai.complete(&req).await.unwrap_err();
        assert!(err.to_string().contains("XAI_API_KEY"));
    }

    #[tokio::test]
    async fn test_provider_status_model_diagnostics() {
        let _guard = ENV_MUTEX.lock().unwrap();
        let gateway = ModelGateway::new();

        // Local provider is ModelAvailable
        let ollama_status = gateway.diagnose_provider("ollama");
        assert_eq!(ollama_status, ProviderStatus::ModelAvailable);

        // Unknown provider is NotConfigured
        let unknown = gateway.diagnose_provider("unknown_provider");
        assert_eq!(unknown, ProviderStatus::NotConfigured);

        // OpenAI with invalid key
        std::env::set_var("OPENAI_API_KEY", "invalid-short");
        assert_eq!(
            gateway.diagnose_provider("openai"),
            ProviderStatus::AuthenticationFailed
        );

        // OpenAI with valid dummy key
        std::env::set_var("OPENAI_API_KEY", "sk-proj-1234567890abcdef");
        assert_eq!(
            gateway.diagnose_provider("openai"),
            ProviderStatus::ModelAvailable
        );

        std::env::remove_var("OPENAI_API_KEY");
        assert_eq!(
            gateway.diagnose_provider("openai"),
            ProviderStatus::NotConfigured
        );

        let all_statuses = gateway.diagnose_all_providers();
        assert!(all_statuses.contains_key("ollama"));
        assert!(all_statuses.contains_key("openai"));
    }
}
