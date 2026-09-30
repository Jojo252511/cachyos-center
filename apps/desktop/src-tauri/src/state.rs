//! Shared application state of the desktop backend.

use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use cachyos_center_service::AppCore;

use crate::helper_client::HelperClient;
use crate::notify::Notifier;

#[derive(Debug)]
pub struct AppState {
    pub core: Arc<AppCore>,
    pub helper: Arc<HelperClient>,
    pub notifier: Arc<Notifier>,
    /// Operations whose end is being watched (history, notification, event).
    pub watched: Arc<Mutex<HashSet<String>>>,
}

impl AppState {
    pub fn new(core: AppCore) -> Self {
        Self {
            core: Arc::new(core),
            helper: Arc::new(HelperClient::default()),
            notifier: Arc::new(Notifier::default()),
            watched: Arc::new(Mutex::new(HashSet::new())),
        }
    }
}
