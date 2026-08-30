pub mod account_setup;
pub mod compose;
pub mod message_list;
pub mod message_view;
pub mod settings;
pub mod sidebar;

pub use account_setup::AccountSetupView;
pub use compose::ComposeView;
pub use message_list::MessageListView;
pub use message_view::MessageViewPane;
pub use settings::SettingsView;
pub use sidebar::{FolderSelection, SidebarView};

