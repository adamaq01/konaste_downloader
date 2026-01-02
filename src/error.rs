use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Failed to convert resource info: {0}")]
    ConvertResourceInfo(#[from] kbinxml::KbinError),
    #[error("Failed to parse resource info: {0}")]
    ParseResourceInfo(#[from] quick_xml::de::DeError),
    #[error("{0}")]
    InternalError(String),
    #[error("{0}")]
    Reqwest(#[from] reqwest::Error),
    #[error("{0}")]
    Io(#[from] std::io::Error),
}
