//! The local filesystem side of syncing: walking a folder into the index,
//! honouring `.pulseignore`, and watching for changes.

pub mod manager;
pub mod scanner;

pub use manager::FolderManager;

use std::path::Path;

/// Per-folder directory reserved for pulse-sync. Its presence also acts as a
/// safety marker: if it disappears (say, an unmounted drive), we refuse to
/// scan rather than record every file as deleted.
pub const INTERNAL_DIR: &str = ".pulse";
/// Gitignore-syntax ignore file, honoured in any directory of a folder.
pub const IGNORE_FILE: &str = ".pulseignore";
/// Prefix for temporary files written while syncing (M2); never indexed.
pub const TEMP_PREFIX: &str = ".pulse-tmp-";

/// Creates the folder marker so the folder can be scanned.
pub fn init_folder_marker(root: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(root.join(INTERNAL_DIR))
}

pub fn has_folder_marker(root: &Path) -> bool {
    root.join(INTERNAL_DIR).is_dir()
}
