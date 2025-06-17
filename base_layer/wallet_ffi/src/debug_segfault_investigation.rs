// Tari JavaScript SDK walletCreate Segfault Investigation
// Debug infrastructure for diagnosing FFI runtime conflicts and memory safety issues

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::runtime::Runtime;
use log::{debug, error, warn, info};

/// Global counter for tracking runtime instances
static RUNTIME_INSTANCE_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Debug context for tracking runtime operations
#[derive(Debug, Clone)]
pub struct RuntimeDiagnostics {
    pub thread_id: thread::ThreadId,
    pub runtime_type: String,
    pub creation_timestamp: u64,
    pub instance_id: u64,
    pub event_loop_conflicts: Vec<String>,
}

impl RuntimeDiagnostics {
    pub fn new(runtime_type: String) -> Self {
        let instance_id = RUNTIME_INSTANCE_COUNTER.fetch_add(1, Ordering::SeqCst);
        let thread_id = thread::current().id();
        let creation_timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        debug!(
            target: "tari::wallet_ffi::debug",
            "Creating runtime diagnostics - ID: {}, Thread: {:?}, Type: {}",
            instance_id, thread_id, runtime_type
        );

        Self {
            thread_id,
            runtime_type,
            creation_timestamp,
            instance_id,
            event_loop_conflicts: Vec::new(),
        }
    }

    pub fn log_event_loop_conflict(&mut self, conflict_description: String) {
        warn!(
            target: "tari::wallet_ffi::debug",
            "Event loop conflict detected - Runtime ID: {}, Conflict: {}",
            self.instance_id, conflict_description
        );
        self.event_loop_conflicts.push(conflict_description);
    }

    pub fn log_runtime_operation(&self, operation: &str) {
        debug!(
            target: "tari::wallet_ffi::debug",
            "Runtime operation - ID: {}, Thread: {:?}, Operation: {}",
            self.instance_id, self.thread_id, operation
        );
    }
}

/// Node.js specific detection utilities
pub struct NodeJsDetector;

impl NodeJsDetector {
    /// Detect if we're running in a Node.js environment
    pub fn is_nodejs_environment() -> bool {
        // Check for common Node.js environment indicators
        std::env::var("NODE_ENV").is_ok() ||
        std::env::var("npm_config_user_config").is_ok() ||
        std::env::var("NODE_PATH").is_ok()
    }

    /// Detect if current thread might be Node.js main thread
    pub fn is_potential_nodejs_thread() -> bool {
        let current_thread = thread::current();
        current_thread.name().map_or(false, |name| {
            name.contains("node") || 
            name.contains("main") ||
            name.contains("event") ||
            name.contains("loop")
        })
    }

    /// Log Node.js environment details
    pub fn log_environment_details() {
        info!(target: "tari::wallet_ffi::debug", "=== Node.js Environment Detection ===");
        
        if Self::is_nodejs_environment() {
            warn!(target: "tari::wallet_ffi::debug", "Node.js environment detected");
            
            if let Ok(node_env) = std::env::var("NODE_ENV") {
                info!(target: "tari::wallet_ffi::debug", "NODE_ENV: {}", node_env);
            }
            
            if let Ok(node_version) = std::env::var("NODE_VERSION") {
                info!(target: "tari::wallet_ffi::debug", "NODE_VERSION: {}", node_version);
            }
        }

        if Self::is_potential_nodejs_thread() {
            warn!(target: "tari::wallet_ffi::debug", "Running on potential Node.js thread");
        }

        info!(target: "tari::wallet_ffi::debug", "Current thread: {:?}", thread::current().id());
        info!(target: "tari::wallet_ffi::debug", "Thread name: {:?}", thread::current().name());
    }
}
