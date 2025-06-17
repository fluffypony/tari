// Alternative Runtime Strategies for Node.js Compatibility
// Implements different approaches to avoid Tokio/Node.js event loop conflicts

use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::Duration;
use tokio::runtime::{Runtime, Builder, Handle};
use log::{debug, error, warn, info};

/// Available runtime strategies for wallet_create
#[derive(Debug, Clone, PartialEq)]
pub enum RuntimeStrategy {
    /// Standard multi-threaded runtime (current implementation, high conflict risk)
    MultiThreaded,
    /// Single-threaded runtime (lower conflict risk)
    SingleThreaded,
    /// Use existing runtime handle if available
    CurrentThread,
    /// Dedicated thread runtime (safest for Node.js)
    DedicatedThread,
    /// Configurable runtime based on environment detection
    Adaptive,
}

impl RuntimeStrategy {
    /// Get the best strategy for the current environment
    pub fn get_recommended_strategy() -> Self {
        // Detect Node.js environment
        let is_nodejs = std::env::var("NODE_ENV").is_ok() ||
                       std::env::var("npm_config_user_config").is_ok() ||
                       std::env::var("NODE_PATH").is_ok();

        if is_nodejs {
            info!(
                target: "tari::wallet_ffi::runtime_strategies",
                "Node.js environment detected, using DedicatedThread strategy"
            );
            RuntimeStrategy::DedicatedThread
        } else {
            // Check if we're already in a Tokio runtime
            if Handle::try_current().is_ok() {
                info!(
                    target: "tari::wallet_ffi::runtime_strategies",
                    "Existing Tokio runtime detected, using CurrentThread strategy"
                );
                RuntimeStrategy::CurrentThread
            } else {
                info!(
                    target: "tari::wallet_ffi::runtime_strategies",
                    "No runtime conflicts detected, using SingleThreaded strategy"
                );
                RuntimeStrategy::SingleThreaded
            }
        }
    }

    /// Create runtime based on strategy
    pub fn create_runtime(&self) -> Result<RuntimeWrapper, String> {
        match self {
            RuntimeStrategy::MultiThreaded => Self::create_multi_threaded_runtime(),
            RuntimeStrategy::SingleThreaded => Self::create_single_threaded_runtime(),
            RuntimeStrategy::CurrentThread => Self::create_current_thread_runtime(),
            RuntimeStrategy::DedicatedThread => Self::create_dedicated_thread_runtime(),
            RuntimeStrategy::Adaptive => {
                let recommended = Self::get_recommended_strategy();
                recommended.create_runtime()
            }
        }
    }

    fn create_multi_threaded_runtime() -> Result<RuntimeWrapper, String> {
        debug!(
            target: "tari::wallet_ffi::runtime_strategies",
            "Creating multi-threaded runtime"
        );

        let runtime = Runtime::new()
            .map_err(|e| format!("Multi-threaded runtime creation failed: {}", e))?;

        Ok(RuntimeWrapper::Owned(runtime))
    }

    fn create_single_threaded_runtime() -> Result<RuntimeWrapper, String> {
        debug!(
            target: "tari::wallet_ffi::runtime_strategies",
            "Creating single-threaded runtime"
        );

        let runtime = Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| format!("Single-threaded runtime creation failed: {}", e))?;

        Ok(RuntimeWrapper::Owned(runtime))
    }

    fn create_current_thread_runtime() -> Result<RuntimeWrapper, String> {
        debug!(
            target: "tari::wallet_ffi::runtime_strategies",
            "Using current thread runtime handle"
        );

        let handle = Handle::try_current()
            .map_err(|e| format!("No current runtime available: {}", e))?;

        Ok(RuntimeWrapper::Handle(handle))
    }

    fn create_dedicated_thread_runtime() -> Result<RuntimeWrapper, String> {
        debug!(
            target: "tari::wallet_ffi::runtime_strategies",
            "Creating dedicated thread runtime"
        );

        let (tx, rx) = mpsc::channel();
        
        let runtime_thread = thread::spawn(move || {
            debug!(
                target: "tari::wallet_ffi::runtime_strategies",
                "Creating runtime on dedicated thread"
            );

            match Runtime::new() {
                Ok(runtime) => {
                    debug!(
                        target: "tari::wallet_ffi::runtime_strategies",
                        "Dedicated thread runtime created successfully"
                    );
                    
                    let handle = runtime.handle().clone();
                    
                    // Send the handle back to the main thread
                    if tx.send(Ok(handle)).is_err() {
                        error!(
                            target: "tari::wallet_ffi::runtime_strategies",
                            "Failed to send runtime handle to main thread"
                        );
                        return;
                    }

                    // Keep the runtime alive by running forever
                    runtime.block_on(async {
                        loop {
                            tokio::time::sleep(Duration::from_secs(1)).await;
                        }
                    });
                }
                Err(e) => {
                    error!(
                        target: "tari::wallet_ffi::runtime_strategies",
                        "Dedicated thread runtime creation failed: {}", e
                    );
                    let _ = tx.send(Err(format!("Dedicated thread runtime failed: {}", e)));
                }
            }
        });

        // Wait for the runtime to be created
        let handle = rx.recv_timeout(Duration::from_secs(10))
            .map_err(|_| "Timeout waiting for dedicated thread runtime".to_string())?
            .map_err(|e| e)?;

        Ok(RuntimeWrapper::DedicatedThread {
            handle,
            _thread: runtime_thread,
        })
    }
}

/// Wrapper for different runtime types
pub enum RuntimeWrapper {
    /// Owned runtime (can use block_on)
    Owned(Runtime),
    /// Handle to existing runtime (can only spawn)
    Handle(Handle),
    /// Dedicated thread runtime with handle
    DedicatedThread {
        handle: Handle,
        _thread: thread::JoinHandle<()>,
    },
}

impl RuntimeWrapper {
    /// Execute a future with the runtime
    pub fn execute<F>(&self, future: F) -> Result<F::Output, String>
    where
        F: std::future::Future + Send + 'static,
        F::Output: Send + 'static,
    {
        match self {
            RuntimeWrapper::Owned(runtime) => {
                debug!(
                    target: "tari::wallet_ffi::runtime_strategies",
                    "Executing future on owned runtime with block_on"
                );
                Ok(runtime.block_on(future))
            }
            RuntimeWrapper::Handle(handle) => {
                debug!(
                    target: "tari::wallet_ffi::runtime_strategies",
                    "Spawning future on runtime handle"
                );
                
                let join_handle = handle.spawn(future);
                
                // We need to block somehow to get the result
                // This is tricky because we can't block_on within an async context
                // For now, we'll spawn and then use a channel to get the result
                let (tx, rx) = mpsc::channel();
                
                handle.spawn(async move {
                    let result = join_handle.await;
                    let _ = tx.send(result);
                });
                
                // Wait for the result (this could potentially block)
                rx.recv_timeout(Duration::from_secs(30))
                    .map_err(|_| "Timeout waiting for async operation".to_string())?
                    .map_err(|e| format!("Async operation failed: {}", e))
            }
            RuntimeWrapper::DedicatedThread { handle, .. } => {
                debug!(
                    target: "tari::wallet_ffi::runtime_strategies",
                    "Executing future on dedicated thread runtime"
                );
                
                let (tx, rx) = mpsc::channel();
                
                handle.spawn(async move {
                    let result = future.await;
                    let _ = tx.send(result);
                });
                
                rx.recv_timeout(Duration::from_secs(60))
                    .map_err(|_| "Timeout waiting for dedicated thread operation".to_string())
            }
        }
    }

    /// Spawn a task on the runtime
    pub fn spawn<F>(&self, future: F) -> Result<tokio::task::JoinHandle<F::Output>, String>
    where
        F: std::future::Future + Send + 'static,
        F::Output: Send + 'static,
    {
        let handle = match self {
            RuntimeWrapper::Owned(runtime) => runtime.handle(),
            RuntimeWrapper::Handle(handle) => handle,
            RuntimeWrapper::DedicatedThread { handle, .. } => handle,
        };

        debug!(
            target: "tari::wallet_ffi::runtime_strategies",
            "Spawning task on runtime"
        );

        Ok(handle.spawn(future))
    }

    /// Get runtime type for debugging
    pub fn get_type(&self) -> &'static str {
        match self {
            RuntimeWrapper::Owned(_) => "owned",
            RuntimeWrapper::Handle(_) => "handle",
            RuntimeWrapper::DedicatedThread { .. } => "dedicated_thread",
        }
    }
}

/// Runtime strategy selector with environment-based configuration
pub struct RuntimeSelector {
    strategy: RuntimeStrategy,
    fallback_strategy: RuntimeStrategy,
}

impl RuntimeSelector {
    /// Create a new runtime selector with environment detection
    pub fn new() -> Self {
        let strategy = RuntimeStrategy::get_recommended_strategy();
        let fallback_strategy = match strategy {
            RuntimeStrategy::DedicatedThread => RuntimeStrategy::SingleThreaded,
            RuntimeStrategy::CurrentThread => RuntimeStrategy::SingleThreaded,
            RuntimeStrategy::SingleThreaded => RuntimeStrategy::MultiThreaded,
            RuntimeStrategy::MultiThreaded => RuntimeStrategy::SingleThreaded,
            RuntimeStrategy::Adaptive => RuntimeStrategy::SingleThreaded,
        };

        info!(
            target: "tari::wallet_ffi::runtime_strategies",
            "Runtime selector created with strategy: {:?}, fallback: {:?}",
            strategy, fallback_strategy
        );

        Self {
            strategy,
            fallback_strategy,
        }
    }

    /// Create a runtime, trying fallback if primary strategy fails
    pub fn create_runtime_with_fallback(&self) -> Result<RuntimeWrapper, String> {
        info!(
            target: "tari::wallet_ffi::runtime_strategies",
            "Attempting to create runtime with strategy: {:?}",
            self.strategy
        );

        match self.strategy.create_runtime() {
            Ok(runtime) => {
                info!(
                    target: "tari::wallet_ffi::runtime_strategies",
                    "Primary runtime strategy succeeded: {:?}",
                    self.strategy
                );
                Ok(runtime)
            }
            Err(e) => {
                warn!(
                    target: "tari::wallet_ffi::runtime_strategies",
                    "Primary runtime strategy failed: {:?}, error: {}, trying fallback: {:?}",
                    self.strategy, e, self.fallback_strategy
                );

                match self.fallback_strategy.create_runtime() {
                    Ok(runtime) => {
                        info!(
                            target: "tari::wallet_ffi::runtime_strategies",
                            "Fallback runtime strategy succeeded: {:?}",
                            self.fallback_strategy
                        );
                        Ok(runtime)
                    }
                    Err(fallback_error) => {
                        error!(
                            target: "tari::wallet_ffi::runtime_strategies",
                            "Both primary and fallback runtime strategies failed. Primary: {} Fallback: {}",
                            e, fallback_error
                        );
                        Err(format!(
                            "All runtime strategies failed. Primary ({}): {}, Fallback ({}): {}",
                            self.strategy.as_str(), e, self.fallback_strategy.as_str(), fallback_error
                        ))
                    }
                }
            }
        }
    }

    /// Get the current strategy
    pub fn get_strategy(&self) -> &RuntimeStrategy {
        &self.strategy
    }

    /// Override the strategy (for testing or manual configuration)
    pub fn set_strategy(&mut self, strategy: RuntimeStrategy) {
        info!(
            target: "tari::wallet_ffi::runtime_strategies",
            "Overriding runtime strategy from {:?} to {:?}",
            self.strategy, strategy
        );
        self.strategy = strategy;
    }
}

impl RuntimeStrategy {
    fn as_str(&self) -> &'static str {
        match self {
            RuntimeStrategy::MultiThreaded => "multi_threaded",
            RuntimeStrategy::SingleThreaded => "single_threaded",
            RuntimeStrategy::CurrentThread => "current_thread",
            RuntimeStrategy::DedicatedThread => "dedicated_thread",
            RuntimeStrategy::Adaptive => "adaptive",
        }
    }
}

impl Default for RuntimeSelector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_runtime_strategy_creation() {
        let selector = RuntimeSelector::new();
        assert!(!matches!(selector.get_strategy(), RuntimeStrategy::Adaptive));
    }

    #[test]
    fn test_multi_threaded_runtime() {
        let strategy = RuntimeStrategy::MultiThreaded;
        let result = strategy.create_runtime();
        assert!(result.is_ok(), "Multi-threaded runtime should work in test environment");
    }

    #[test]
    fn test_single_threaded_runtime() {
        let strategy = RuntimeStrategy::SingleThreaded;
        let result = strategy.create_runtime();
        assert!(result.is_ok(), "Single-threaded runtime should work in test environment");
    }

    #[test]
    fn test_runtime_selector_fallback() {
        let selector = RuntimeSelector::new();
        let result = selector.create_runtime_with_fallback();
        assert!(result.is_ok(), "Runtime selector should succeed with fallback");
    }

    #[tokio::test]
    async fn test_current_thread_runtime() {
        let strategy = RuntimeStrategy::CurrentThread;
        let result = strategy.create_runtime();
        assert!(result.is_ok(), "Current thread runtime should work when in Tokio context");
    }
}
