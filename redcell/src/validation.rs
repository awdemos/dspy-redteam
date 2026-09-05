use crate::error::{AppError, AppResult};
use regex::Regex;
use validator::ValidateEmail;

pub fn validate_email(email: &str) -> AppResult<()> {
    if !ValidateEmail::validate_email(&email) {
        return Err(AppError::BadRequest("invalid email address".to_string()));
    }
    Ok(())
}

pub fn validate_password(password: &str) -> AppResult<()> {
    if password.len() < 12 {
        return Err(AppError::BadRequest(
            "password must be at least 12 characters".to_string(),
        ));
    }
    if !password.chars().any(|c| c.is_ascii_uppercase()) {
        return Err(AppError::BadRequest(
            "password must contain an uppercase letter".to_string(),
        ));
    }
    if !password.chars().any(|c| c.is_ascii_lowercase()) {
        return Err(AppError::BadRequest(
            "password must contain a lowercase letter".to_string(),
        ));
    }
    if !password.chars().any(|c| c.is_ascii_digit()) {
        return Err(AppError::BadRequest(
            "password must contain a digit".to_string(),
        ));
    }
    Ok(())
}

pub fn validate_intent(intent: &str) -> AppResult<()> {
    if intent.is_empty() {
        return Err(AppError::BadRequest("intent is required".to_string()));
    }
    if intent.len() > 2000 {
        return Err(AppError::BadRequest(
            "intent must not exceed 2000 characters".to_string(),
        ));
    }
    Ok(())
}

pub fn validate_target_model(model: &str) -> AppResult<()> {
    if model.is_empty() {
        return Err(AppError::BadRequest("target_model is required".to_string()));
    }
    if model.len() > 128 {
        return Err(AppError::BadRequest(
            "target_model must not exceed 128 characters".to_string(),
        ));
    }
    static MODEL_RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    let re = MODEL_RE.get_or_init(|| Regex::new(r"^[a-zA-Z0-9_./:-]+$").unwrap());
    if !re.is_match(model) {
        return Err(AppError::BadRequest(
            "target_model contains invalid characters".to_string(),
        ));
    }
    Ok(())
}

pub fn validate_layers(layers: i32) -> AppResult<i32> {
    if !(1..=10).contains(&layers) {
        return Err(AppError::BadRequest(
            "layers must be between 1 and 10".to_string(),
        ));
    }
    Ok(layers)
}
