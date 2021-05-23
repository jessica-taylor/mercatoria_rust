//! Message-logging functionality.

use super::role::Role;
use super::Network;

/// Logs messages.
pub struct Log {
    messages: Vec<String>,
}

impl<N: Network> Role<N> for Log {}

impl Log {
    /// Creates a new `Log`.
    fn new() -> Log {
        Log {
            messages: Vec::new(),
        }
    }

    /// Writes to the log.
    fn write(&mut self, msg: String) {
        self.messages.push(msg);
    }

    /// Gets a reference to the logged messages.
    fn get_messages(&self) -> &Vec<String> {
        &self.messages
    }
}
