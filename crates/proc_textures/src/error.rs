use std::fmt::{Display, Formatter};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub enum TextureDataErrorKind {
    Io,
    Parse,
    Validate,
    Generate,
    Export,
}

#[derive(Debug, Clone)]
pub struct TextureDataError {
    pub kind: TextureDataErrorKind,
    pub file: PathBuf,
    pub path: Option<String>,
    pub message: String,
}

impl TextureDataError {
    pub fn new(
        kind: TextureDataErrorKind,
        file: impl Into<PathBuf>,
        path: Option<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            file: file.into(),
            path,
            message: message.into(),
        }
    }

    pub fn io(file: impl Into<PathBuf>, message: impl Into<String>) -> Self {
        Self::new(TextureDataErrorKind::Io, file, None, message)
    }

    pub fn parse(
        file: impl Into<PathBuf>,
        path: Option<String>,
        message: impl Into<String>,
    ) -> Self {
        Self::new(TextureDataErrorKind::Parse, file, path, message)
    }

    pub fn validate(
        file: impl Into<PathBuf>,
        path: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self::new(
            TextureDataErrorKind::Validate,
            file,
            Some(path.into()),
            message,
        )
    }

    pub fn generate(
        file: impl Into<PathBuf>,
        path: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self::new(
            TextureDataErrorKind::Generate,
            file,
            Some(path.into()),
            message,
        )
    }

    pub fn export(
        file: impl Into<PathBuf>,
        path: Option<String>,
        message: impl Into<String>,
    ) -> Self {
        Self::new(TextureDataErrorKind::Export, file, path, message)
    }

    pub fn with_prefix(&self, prefix: &Path) -> String {
        match &self.path {
            Some(path) => format!("{}:{path}: {}", prefix.display(), self.message),
            None => format!("{}: {}", prefix.display(), self.message),
        }
    }
}

impl Display for TextureDataError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match &self.path {
            Some(path) => write!(f, "{}:{path}: {}", self.file.display(), self.message),
            None => write!(f, "{}: {}", self.file.display(), self.message),
        }
    }
}

impl std::error::Error for TextureDataError {}
