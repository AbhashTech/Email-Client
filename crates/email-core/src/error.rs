use thiserror::Error;

#[derive(Error, Debug)]
pub enum EmailError {
    #[error("Database error: {0}")]
    Database(String),

    #[error("Keyring credential error: {0}")]
    Keyring(String),

    #[error("Authentication failed: {0}")]
    Auth(String),

    #[error("IMAP error: {0}")]
    Imap(String),

    #[error("SMTP error: {0}")]
    Smtp(String),

    #[error("MIME parse error: {0}")]
    MimeParse(String),

    #[error("Network connection error: {0}")]
    Network(String),

    #[error("Account not found: {0}")]
    AccountNotFound(String),

    #[error("Folder not found: {0}")]
    FolderNotFound(String),

    #[error("Message not found: {0}")]
    MessageNotFound(String),

    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    #[error("Encryption error: {0}")]
    Encryption(String),

    #[error("Operation cancelled")]
    Cancelled,
}

pub type Result<T> = std::result::Result<T, EmailError>;
