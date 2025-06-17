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

/// Memory alignment checker for FFI boundary safety
pub struct MemoryAlignmentChecker;

impl MemoryAlignmentChecker {
    /// Check if a pointer is properly aligned for the given type
    pub fn check_alignment<T>(ptr: *const T) -> bool {
        let alignment = std::mem::align_of::<T>();
        let addr = ptr as usize;
        addr % alignment == 0
    }

    /// Log alignment check results
    pub fn log_alignment_check<T>(ptr: *const T, type_name: &str) {
        let is_aligned = Self::check_alignment(ptr);
        if !is_aligned {
            error!(
                target: "tari::wallet_ffi::debug",
                "Memory alignment violation - Type: {}, Address: {:p}, Required alignment: {}",
                type_name, ptr, std::mem::align_of::<T>()
            );
        } else {
            debug!(
                target: "tari::wallet_ffi::debug",
                "Memory alignment OK - Type: {}, Address: {:p}",
                type_name, ptr
            );
        }
    }
}

/// Runtime safety wrapper for detecting conflicts
pub struct SafeRuntimeWrapper {
    diagnostics: RuntimeDiagnostics,
    runtime: Option<Runtime>,
}

impl SafeRuntimeWrapper {
    pub fn new(runtime_type: String) -> Self {
        let diagnostics = RuntimeDiagnostics::new(runtime_type);
        Self {
            diagnostics,
            runtime: None,
        }
    }

    /// Create runtime with extensive diagnostic logging
    pub fn create_runtime(&mut self) -> Result<(), String> {
        self.diagnostics.log_runtime_operation("Attempting runtime creation");

        // Check if we're already in a Tokio runtime context
        if tokio::runtime::Handle::try_current().is_ok() {
            let conflict = "Detected existing Tokio runtime - potential conflict with Node.js event loop".to_string();
            self.diagnostics.log_event_loop_conflict(conflict);
        }

        // Check thread information
        let current_thread = thread::current();
        info!(
            target: "tari::wallet_ffi::debug",
            "Creating runtime on thread: {:?}, Name: {:?}",
            current_thread.id(),
            current_thread.name()
        );

        match Runtime::new() {
            Ok(runtime) => {
                self.diagnostics.log_runtime_operation("Runtime creation successful");
                self.runtime = Some(runtime);
                Ok(())
            }
            Err(e) => {
                error!(
                    target: "tari::wallet_ffi::debug",
                    "Runtime creation failed - ID: {}, Error: {}",
                    self.diagnostics.instance_id, e
                );
                Err(format!("Runtime creation failed: {}", e))
            }
        }
    }

    /// Execute async operation with diagnostic logging
    pub fn block_on<F>(&self, future: F) -> Result<F::Output, String>
    where
        F: std::future::Future,
    {
        match &self.runtime {
            Some(runtime) => {
                self.diagnostics.log_runtime_operation("Executing block_on operation");
                
                // Log potential risks
                if thread::current().name().map_or(false, |name| name.contains("node")) {
                    warn!(
                        target: "tari::wallet_ffi::debug",
                        "Running block_on on potential Node.js thread - this may cause deadlocks"
                    );
                }

                let result = runtime.block_on(future);
                self.diagnostics.log_runtime_operation("block_on operation completed");
                Ok(result)
            }
            None => Err("Runtime not initialized".to_string()),
        }
    }

    /// Spawn async task with diagnostic logging
    pub fn spawn<F>(&self, future: F) -> Result<tokio::task::JoinHandle<F::Output>, String>
    where
        F: std::future::Future + Send + 'static,
        F::Output: Send + 'static,
    {
        match &self.runtime {
            Some(runtime) => {
                self.diagnostics.log_runtime_operation("Spawning async task");
                let handle = runtime.spawn(future);
                Ok(handle)
            }
            None => Err("Runtime not initialized".to_string()),
        }
    }

    pub fn get_diagnostics(&self) -> &RuntimeDiagnostics {
        &self.diagnostics
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_runtime_diagnostics_creation() {
        let diagnostics = RuntimeDiagnostics::new("test_runtime".to_string());
        assert_eq!(diagnostics.runtime_type, "test_runtime");
        assert_eq!(diagnostics.thread_id, thread::current().id());
        assert!(diagnostics.event_loop_conflicts.is_empty());
    }

    #[test]
    fn test_memory_alignment_checker() {
        let value: u64 = 42;
        assert!(MemoryAlignmentChecker::check_alignment(&value));
        
        // Test with a misaligned pointer (this is unsafe but for testing)
        let misaligned_ptr = ((&value as *const u64 as usize) + 1) as *const u64;
        assert!(!MemoryAlignmentChecker::check_alignment(misaligned_ptr));
    }

    #[tokio::test]
    async fn test_safe_runtime_wrapper() {
        let mut wrapper = SafeRuntimeWrapper::new("test_runtime".to_string());
        assert!(wrapper.create_runtime().is_ok());
        
        let result = wrapper.block_on(async { 42 });
        assert_eq!(result.unwrap(), 42);
    }
}
