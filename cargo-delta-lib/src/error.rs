pub type Result<T> = core::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Failed to execute cargo command: {0}")]
    CargoExecution(#[from] std::io::Error),

    #[error("Cargo command failed: {0}")]
    CargoCommand(String),

    #[error("Git operation failed: {0}")]
    Git(String),

    #[error("Failed to parse cargo metadata JSON: {0}")]
    CargoJsonParse(#[from] serde_json::Error),

    #[error("Failed to read config file: {0}")]
    ConfigRead(std::io::Error),

    #[error("Failed to parse config file: {0}")]
    ConfigParse(#[from] toml::de::Error),

    #[error("Failed to read JSON file '{file}': {source}")]
    JsonFileRead {
        file: String,
        #[source]
        source: std::io::Error,
    },

    #[error("Failed to parse JSON file '{file}': {source}")]
    JsonFileParse {
        file: String,
        #[source]
        source: serde_json::Error,
    },

    #[error(transparent)]
    Syn(#[from] syn::Error),

    #[error("{0}")]
    Other(String),
}

impl From<&str> for Error {
    fn from(msg: &str) -> Self {
        Self::Other(msg.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_from_str() {
        let err = Error::from("something failed");
        assert_eq!(err.to_string(), "something failed");
    }

    #[test]
    fn error_display_variants() {
        let err = Error::CargoCommand("bad".to_string());
        assert_eq!(err.to_string(), "Cargo command failed: bad");

        let err = Error::Git("no remote".to_string());
        assert_eq!(err.to_string(), "Git operation failed: no remote");
    }
}
