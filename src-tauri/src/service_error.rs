//! 業務処理層のエラー分類。UI へは利用者向けメッセージのみ渡し、内部原因は保持する。

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServiceError {
    LockPoisoned,
    NotFound { resource: String },
    Validation { message: String },
    External { message: String, cause: String },
    Internal { message: String, cause: String },
}

impl ServiceError {
    pub fn validation(message: impl Into<String>) -> Self {
        Self::Validation {
            message: message.into(),
        }
    }

    pub fn external(user_message: impl Into<String>, cause: impl std::fmt::Display) -> Self {
        Self::External {
            message: user_message.into(),
            cause: cause.to_string(),
        }
    }

    pub fn internal(user_message: impl Into<String>, cause: impl std::fmt::Display) -> Self {
        Self::Internal {
            message: user_message.into(),
            cause: cause.to_string(),
        }
    }

    /// IPC / UI 向けの短いメッセージ。
    pub fn user_message(&self) -> String {
        match self {
            Self::LockPoisoned => "内部エラー: データベースロック".into(),
            Self::NotFound { resource } => format!("{resource}が見つかりません"),
            Self::Validation { message } => message.clone(),
            Self::External { message, .. } | Self::Internal { message, .. } => message.clone(),
        }
    }

    /// ログや診断向けの内部原因。利用者向け表示には使わない。
    pub fn internal_cause(&self) -> Option<&str> {
        match self {
            Self::External { cause, .. } | Self::Internal { cause, .. } => Some(cause),
            _ => None,
        }
    }
}

impl std::fmt::Display for ServiceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.user_message())
    }
}

impl From<ServiceError> for String {
    fn from(value: ServiceError) -> String {
        value.user_message()
    }
}

pub type ServiceResult<T> = Result<T, ServiceError>;

pub fn lock_db(
    state: &crate::app_state::AppState,
) -> ServiceResult<std::sync::MutexGuard<'_, rusqlite::Connection>> {
    state.db.lock().map_err(|_| ServiceError::LockPoisoned)
}

#[cfg(test)]
mod tests {
    use super::ServiceError;

    #[test]
    fn user_message_hides_internal_cause() {
        let err = ServiceError::external("接続に失敗しました", "timeout after 30s");
        assert_eq!(err.user_message(), "接続に失敗しました");
        assert_eq!(err.internal_cause(), Some("timeout after 30s"));
    }

    #[test]
    fn converts_to_ipc_string_via_display() {
        let err = ServiceError::validation("トークンが空です");
        assert_eq!(err.to_string(), "トークンが空です");
    }
}
