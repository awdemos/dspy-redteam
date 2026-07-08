use crate::config::LlmConfig;
use crate::error::{AppError, AppResult};
use async_openai::{
    Client,
    config::OpenAIConfig,
    types::{
        ChatCompletionRequestSystemMessage, ChatCompletionRequestUserMessage,
        ChatCompletionRequestUserMessageContent, CreateChatCompletionRequestArgs,
    },
};
use serde::de::DeserializeOwned;

#[derive(Debug, Clone, Default)]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
}

impl TokenUsage {
    pub fn total(&self) -> u32 {
        self.prompt_tokens + self.completion_tokens
    }
}

#[derive(Clone)]
pub struct LlmClient {
    attack_client: Client<OpenAIConfig>,
    target_client: Client<OpenAIConfig>,
    pub attack_model: String,
    pub judge_model: String,
}

impl LlmClient {
    pub fn new(config: &LlmConfig) -> Self {
        let attack_cfg = OpenAIConfig::new()
            .with_api_key(config.api_key.clone())
            .with_api_base(config.base_url.clone());

        let target_cfg = OpenAIConfig::new()
            .with_api_key(
                config
                    .target_api_key
                    .clone()
                    .unwrap_or_else(|| config.api_key.clone()),
            )
            .with_api_base(config.target_base_url.clone());

        Self {
            attack_client: Client::with_config(attack_cfg),
            target_client: Client::with_config(target_cfg),
            attack_model: config.attack_model.clone(),
            judge_model: config.judge_model.clone(),
        }
    }

    pub async fn chat_with_usage(
        &self,
        model: &str,
        system: &str,
        user: &str,
    ) -> AppResult<(String, TokenUsage)> {
        let request = CreateChatCompletionRequestArgs::default()
            .model(model)
            .max_tokens(512u32)
            .temperature(0.0)
            .messages([
                ChatCompletionRequestSystemMessage {
                    content: system.into(),
                    name: None,
                }
                .into(),
                ChatCompletionRequestUserMessage {
                    content: ChatCompletionRequestUserMessageContent::Text(user.into()),
                    name: None,
                }
                .into(),
            ])
            .build()
            .map_err(|e| AppError::Llm(e.to_string()))?;

        let client = if model == self.attack_model || model == self.judge_model {
            &self.attack_client
        } else {
            &self.target_client
        };

        let response = client
            .chat()
            .create(request)
            .await
            .map_err(|e| AppError::Llm(e.to_string()))?;

        let text = response
            .choices
            .into_iter()
            .next()
            .and_then(|c| c.message.content)
            .ok_or_else(|| AppError::Llm("empty response".to_string()))?;

        let usage = response
            .usage
            .map_or_else(TokenUsage::default, |u| TokenUsage {
                prompt_tokens: u.prompt_tokens,
                completion_tokens: u.completion_tokens,
            });

        Ok((text, usage))
    }

    pub async fn chat(&self, model: &str, system: &str, user: &str) -> AppResult<String> {
        Ok(self.chat_with_usage(model, system, user).await?.0)
    }

    pub async fn chat_structured<T: DeserializeOwned>(
        &self,
        model: &str,
        system: &str,
        user: &str,
    ) -> AppResult<T> {
        let (text, _) = self.chat_with_usage(model, system, user).await?;
        serde_json::from_str(&text)
            .or_else(|_| {
                extract_json_object(&text).and_then(|json| {
                    serde_json::from_str(&json).map_err(|e| {
                        AppError::Llm(format!("failed to parse structured output: {}", e))
                    })
                })
            })
            .map_err(|e| AppError::Llm(format!("failed to parse structured output: {}", e)))
    }

    pub async fn attack(&self, system: &str, user: &str) -> AppResult<String> {
        self.chat(&self.attack_model, system, user).await
    }

    pub async fn judge(&self, system: &str, user: &str) -> AppResult<String> {
        self.chat(&self.judge_model, system, user).await
    }

    pub async fn target(&self, model: &str, system: &str, user: &str) -> AppResult<String> {
        self.chat(model, system, user).await
    }
}

fn extract_json_object(text: &str) -> AppResult<String> {
    let start = text
        .find('{')
        .ok_or_else(|| AppError::Llm("no json found".to_string()))?;
    let end = text
        .rfind('}')
        .ok_or_else(|| AppError::Llm("no json found".to_string()))?;
    Ok(text[start..=end].to_string())
}
