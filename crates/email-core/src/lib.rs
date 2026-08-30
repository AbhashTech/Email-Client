pub mod error;
pub mod events;
pub mod models;

pub use error::{EmailError, Result};
pub use events::{SyncCommand, SyncEvent};
pub use models::*;
