//! Message-logging functionality.

use super::role::Role;
use super::Network;
use std::sync::RwLock;

/// Logs messages.
pub struct Log {
    messages: RwLock<Vec<String>>,
}

impl<N: Network> Role<N> for Log {}

impl Log {
    /// Creates a new `Log`.
    fn new() -> Log {
        Log {
            messages: RwLock::new(Vec::new()),
        }
    }

    /// Writes to the log.
    fn write(&self, msg: String) {
        let mut msgs = self.messages.write().unwrap();
        (*msgs).push(msg);
    }

    /// Gets a reference to the logged messages.
    fn get_messages(&self) -> Vec<String> {
        let msgs = self.messages.read().unwrap();
        (*msgs).clone()
    }
}
