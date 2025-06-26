pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Git(#[from] git2::Error),

    #[error("Failed to execute cargo command: {0}")]
    CargoExecution(#[from] std::io::Error),

    #[error("Cargo command failed: {0}")]
    CargoCommand(String),

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
    SynError(#[from] syn::Error),

    #[error("{0}")]
    Other(String),
}

impl From<&str> for Error {
    fn from(msg: &str) -> Self {
        Error::Other(msg.to_string())
    }
}
