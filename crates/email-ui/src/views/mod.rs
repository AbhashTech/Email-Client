pub mod account_setup;
pub mod command_palette;
pub mod compose;
pub mod message_list;
pub mod message_view;
pub mod settings;
pub mod sidebar;

pub use account_setup::AccountSetupView;
pub use command_palette::{CommandPalette, PaletteAction, PaletteItem};
#[allow(unused_imports)]
pub use compose::{ComposeFormat, ComposeView};
pub use message_list::MessageListView;
pub use message_view::MessageViewPane;
pub use settings::SettingsView;
pub use sidebar::{FolderSelection, SidebarView};


