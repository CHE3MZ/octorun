//! Shared cancellation and cleanup helpers for commands.

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::error::{Error, Result};

/// Install a Ctrl+C handler and return the flag it sets.
///
/// # Errors
///
/// Returns an error if the handler cannot be installed.
pub(crate) fn cancel_flag() -> Result<Arc<AtomicBool>> {
    let cancelled = Arc::new(AtomicBool::new(false));
    let handler = Arc::clone(&cancelled);

    ctrlc::set_handler(move || {
        handler.store(true, Ordering::SeqCst);
    })
    .map_err(|error| Error::Command(format!("could not install Ctrl+C handler: {error}")))?;

    Ok(cancelled)
}

/// Removes a runner directory if setup does not finish.
pub(crate) struct DirGuard {
    dir: PathBuf,
    armed: bool,
}

impl DirGuard {
    #[must_use]
    pub(crate) fn new(dir: PathBuf) -> Self {
        let armed = !dir.exists();
        Self { dir, armed }
    }

    pub(crate) fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for DirGuard {
    fn drop(&mut self) {
        if self.armed {
            let _ = fs::remove_dir_all(&self.dir);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{DirGuard, cancel_flag};

    #[test]
    fn a_second_handler_installation_fails() {
        let _ = cancel_flag();
        assert!(cancel_flag().is_err());
    }

    #[test]
    fn guard_removes_only_new_directories() {
        let dir = tempfile::tempdir().unwrap();

        let fresh = dir.path().join("fresh");
        {
            let _guard = DirGuard::new(fresh.clone());
            fs::create_dir_all(&fresh).unwrap();
        }
        assert!(!fresh.exists());

        let existing = dir.path().join("existing");
        fs::create_dir_all(&existing).unwrap();
        {
            let _guard = DirGuard::new(existing.clone());
        }
        assert!(existing.exists());
    }
}
