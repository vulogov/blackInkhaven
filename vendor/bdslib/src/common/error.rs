pub use easy_error::Error;
pub use easy_error::err_msg;

/// Shared result type used across all bdslib modules.
pub type Result<T> = std::result::Result<T, Error>;
