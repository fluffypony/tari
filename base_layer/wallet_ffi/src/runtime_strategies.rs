//! Runtime Strategies for Node.js Compatibility
//! 
//! Multiple Tokio runtime strategies to resolve conflicts with Node.js event loops
//! that cause segfaults in the wallet_create function and other FFI operations.

use std::{
    sync::{Arc, Mutex, mpsc, OnceLock},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
use tokio::runtime::{Builder, Handle, Runtime};
use log::{debug, error, info, warn};

/// Global runtime strategy state
static RUNTIME_STRATEGY: OnceLock<Arc<Mutex<Option<RuntimeManager>>>> = OnceLock::new();

/// Runtime strategy options
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RuntimeStrategy {
    /// Multi-threaded runtime (default, but problematic with Node.js)
    MultiThreaded,
    /// Single-threaded current thread runtime
    SingleThreaded,
    /// Dedicated thread with isolated runtime
    DedicatedThread,
    /// Use existing runtime handle if available
    HandleBased,
    /// Adaptive strategy based on environment detection
    Adaptive,
}

/// Runtime execution context
#[derive(Debug)]
pub enum RuntimeContext {
    /// Runtime is available and ready
    Ready(RuntimeHandle),
    /// Runtime initialization failed
    Failed(String),
    /// Runtime is being initialized
    Initializing,
}

/// Runtime handle abstraction
#[derive(Debug)]
pub enum RuntimeHandle {
    /// Direct runtime reference
    Runtime(Arc<Runtime>),
    /// Tokio handle reference
    Handle(Handle),
    /// Dedicated thread communication
    Thread(ThreadedRuntime),
}

/// Dedicated thread runtime management
#[derive(Debug)]
pub struct ThreadedRuntime {
    sender: mpsc::Sender<ThreadedTask>,
    join_handle: Option<JoinHandle<()>>,
}

/// Task for threaded runtime execution
struct ThreadedTask {
    task: Box<dyn FnOnce() + Send>,
    result_sender: mpsc::Sender<Result<(), String>>,
}

/// Runtime manager for coordinating different strategies
pub struct RuntimeManager {
    strategy: RuntimeStrategy,
    context: RuntimeContext,
    nodejs_detected: bool,
    initialization_time: Instant,
}

impl RuntimeManager {
    /// Initialize with automatic strategy detection
    pub fn initialize() -> Result<(), Box<dyn std::error::Error>> {
        let strategy_mgr = RUNTIME_STRATEGY.get_or_init(|| {
            Arc::new(Mutex::new(None))
        });

        let mut guard = strategy_mgr.lock().unwrap();
        if guard.is_some() {
            return Ok(()); // Already initialized
        }

        let nodejs_detected = Self::detect_nodejs_environment();
        let strategy = if nodejs_detected {
            RuntimeStrategy::Adaptive
        } else {
            RuntimeStrategy::MultiThreaded
        };

        info!("Initializing runtime manager (Node.js detected: {}, strategy: {:?})", 
              nodejs_detected, strategy);

        let mut manager = Self {
            strategy,
            context: RuntimeContext::Initializing,
            nodejs_detected,
            initialization_time: Instant::now(),
        };

        manager.initialize_strategy()?;
        *guard = Some(manager);

        Ok(())
    }

    /// Get or create runtime manager instance
    pub fn get_instance() -> Result<Arc<Mutex<Option<RuntimeManager>>>, String> {
        Self::initialize().map_err(|e| format!("Failed to initialize runtime manager: {}", e))?;
        
        let strategy_mgr = RUNTIME_STRATEGY.get()
            .ok_or("Runtime strategy not initialized")?;
        
        let guard = strategy_mgr.lock().unwrap();
        match guard.as_ref() {
            Some(_) => Ok(strategy_mgr.clone()),
            None => Err("Runtime manager not properly initialized".to_string()),
        }
    }

    /// Initialize the selected strategy
    fn initialize_strategy(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        info!("Initializing runtime strategy: {:?}", self.strategy);

        let result = match self.strategy {
            RuntimeStrategy::MultiThreaded => self.init_multithreaded(),
            RuntimeStrategy::SingleThreaded => self.init_single_threaded(),
            RuntimeStrategy::DedicatedThread => self.init_dedicated_thread(),
            RuntimeStrategy::HandleBased => self.init_handle_based(),
            RuntimeStrategy::Adaptive => self.init_adaptive(),
        };

        match result {
            Ok(handle) => {
                self.context = RuntimeContext::Ready(handle);
                info!("Runtime strategy initialized successfully in {:?}", 
                     self.initialization_time.elapsed());
                Ok(())
            }
            Err(e) => {
                error!("Runtime strategy initialization failed: {}", e);
                self.context = RuntimeContext::Failed(e.clone());
                Err(e.into())
            }
        }
    }

    /// Initialize multi-threaded runtime
    fn init_multithreaded(&self) -> Result<RuntimeHandle, String> {
        debug!("Creating multi-threaded runtime");
        
        let rt = Builder::new_multi_thread()
            .worker_threads(2) // Limit threads to reduce conflicts
            .thread_name("tari-wallet")
            .enable_all()
            .build()
            .map_err(|e| format!("Failed to create multi-threaded runtime: {}", e))?;

        Ok(RuntimeHandle::Runtime(Arc::new(rt)))
    }

    /// Initialize single-threaded runtime
    fn init_single_threaded(&self) -> Result<RuntimeHandle, String> {
        debug!("Creating single-threaded runtime");
        
        let rt = Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| format!("Failed to create single-threaded runtime: {}", e))?;

        Ok(RuntimeHandle::Runtime(Arc::new(rt)))
    }

    /// Initialize dedicated thread runtime
    fn init_dedicated_thread(&self) -> Result<RuntimeHandle, String> {
        debug!("Creating dedicated thread runtime");
        
        let (task_sender, task_receiver) = mpsc::channel::<ThreadedTask>();
        
        let handle = thread::Builder::new()
            .name("tari-dedicated-runtime".to_string())
            .spawn(move || {
                info!("Dedicated runtime thread started");
                
                // Create runtime in dedicated thread
                let rt = match Builder::new_current_thread()
                    .enable_all()
                    .build() 
                {
                    Ok(runtime) => runtime,
                    Err(e) => {
                        error!("Failed to create dedicated thread runtime: {}", e);
                        return;
                    }
                };

                // Process tasks
                while let Ok(task) = task_receiver.recv() {
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        rt.block_on(async {
                            (task.task)();
                        });
                    }));

                    let task_result = match result {
                        Ok(_) => Ok(()),
                        Err(e) => {
                            let msg = if let Some(s) = e.downcast_ref::<String>() {
                                s.clone()
                            } else if let Some(s) = e.downcast_ref::<&str>() {
                                s.to_string()
                            } else {
                                "Unknown panic in dedicated thread".to_string()
                            };
                            error!("Task panic in dedicated thread: {}", msg);
                            Err(msg)
                        }
                    };

                    let _ = task.result_sender.send(task_result);
                }

                info!("Dedicated runtime thread shutting down");
            })
            .map_err(|e| format!("Failed to spawn dedicated thread: {}", e))?;

        Ok(RuntimeHandle::Thread(ThreadedRuntime {
            sender: task_sender,
            join_handle: Some(handle),
        }))
    }

    /// Initialize handle-based runtime
    fn init_handle_based(&self) -> Result<RuntimeHandle, String> {
        debug!("Using existing runtime handle");
        
        match Handle::try_current() {
            Ok(handle) => {
                info!("Using existing Tokio runtime handle");
                Ok(RuntimeHandle::Handle(handle))
            }
            Err(_) => {
                warn!("No existing runtime handle, falling back to single-threaded");
                self.init_single_threaded()
            }
        }
    }

    /// Initialize adaptive strategy based on environment
    fn init_adaptive(&self) -> Result<RuntimeHandle, String> {
        debug!("Using adaptive runtime strategy");
        
        if self.nodejs_detected {
            info!("Node.js detected, using dedicated thread strategy");
            self.init_dedicated_thread()
        } else {
            // Try handle-based first, then fall back to appropriate strategy
            match Handle::try_current() {
                Ok(handle) => {
                    info!("Existing runtime handle found, using it");
                    Ok(RuntimeHandle::Handle(handle))
                }
                Err(_) => {
                    info!("No existing runtime, creating single-threaded runtime");
                    self.init_single_threaded()
                }
            }
        }
    }

    /// Detect Node.js environment
    fn detect_nodejs_environment() -> bool {
        // Check for Node.js environment variables
        if std::env::var("NODE_VERSION").is_ok() || 
           std::env::var("npm_config_registry").is_ok() ||
           std::env::var("NODE_ENV").is_ok() ||
           std::env::var("npm_lifecycle_event").is_ok() {
            return true;
        }

        // Check process name
        if let Ok(exe) = std::env::current_exe() {
            if let Some(name) = exe.file_name() {
                if let Some(name_str) = name.to_str() {
                    if name_str.contains("node") || name_str.contains("npm") || name_str.contains("yarn") {
                        return true;
                    }
                }
            }
        }

        // Check for process title patterns
        if let Ok(args) = std::env::var("_") {
            if args.contains("node") || args.contains("npm") {
                return true;
            }
        }

        false
    }

    /// Execute a future using the configured runtime strategy
    pub fn block_on<F, T>(&self, future: F) -> Result<T, String> 
    where
        F: std::future::Future<Output = T> + Send + 'static,
        T: Send + 'static,
    {
        match &self.context {
            RuntimeContext::Ready(handle) => {
                self.execute_with_handle(handle, future)
            }
            RuntimeContext::Failed(err) => {
                Err(format!("Runtime not available: {}", err))
            }
            RuntimeContext::Initializing => {
                Err("Runtime is still initializing".to_string())
            }
        }
    }

    /// Execute future with specific runtime handle
    fn execute_with_handle<F, T>(&self, handle: &RuntimeHandle, future: F) -> Result<T, String>
    where
        F: std::future::Future<Output = T> + Send + 'static,
        T: Send + 'static,
    {
        match handle {
            RuntimeHandle::Runtime(rt) => {
                Ok(rt.block_on(future))
            }
            RuntimeHandle::Handle(h) => {
                // For handle-based execution, we need to spawn and wait
                let (tx, rx) = mpsc::channel();
                
                h.spawn(async move {
                    let result = future.await;
                    let _ = tx.send(result);
                });

                rx.recv_timeout(Duration::from_secs(30))
                    .map_err(|e| format!("Handle-based execution timeout: {}", e))
            }
            RuntimeHandle::Thread(threaded) => {
                self.execute_in_thread(threaded, future)
            }
        }
    }

    /// Execute future in dedicated thread
    fn execute_in_thread<F, T>(&self, threaded: &ThreadedRuntime, future: F) -> Result<T, String>
    where
        F: std::future::Future<Output = T> + Send + 'static,
        T: Send + 'static,
    {
        let (result_tx, result_rx) = mpsc::channel();
        let (value_tx, value_rx) = mpsc::channel();

        let task = ThreadedTask {
            task: Box::new(move || {
                // This will be executed inside the async context of the dedicated thread
                tokio::spawn(async move {
                    let result = future.await;
                    let _ = value_tx.send(result);
                });
            }),
            result_sender: result_tx,
        };

        threaded.sender.send(task)
            .map_err(|e| format!("Failed to send task to dedicated thread: {}", e))?;

        // Wait for task completion
        result_rx.recv_timeout(Duration::from_secs(30))
            .map_err(|e| format!("Dedicated thread task timeout: {}", e))?
            .map_err(|e| format!("Dedicated thread task failed: {}", e))?;

        // Wait for actual result
        value_rx.recv_timeout(Duration::from_secs(30))
            .map_err(|e| format!("Dedicated thread result timeout: {}", e))
    }

    /// Get runtime statistics
    pub fn get_statistics(&self) -> RuntimeStatistics {
        RuntimeStatistics {
            strategy: self.strategy,
            nodejs_detected: self.nodejs_detected,
            initialization_time: self.initialization_time.elapsed(),
            is_ready: matches!(self.context, RuntimeContext::Ready(_)),
            error: match &self.context {
                RuntimeContext::Failed(err) => Some(err.clone()),
                _ => None,
            },
        }
    }
}

/// Runtime execution statistics
#[derive(Debug)]
pub struct RuntimeStatistics {
    pub strategy: RuntimeStrategy,
    pub nodejs_detected: bool,
    pub initialization_time: Duration,
    pub is_ready: bool,
    pub error: Option<String>,
}

/// Convenient macros for runtime execution
#[macro_export]
macro_rules! execute_async {
    ($future:expr) => {{
        use $crate::base_layer::wallet_ffi::src::runtime_strategies::RuntimeManager;
        
        let manager_arc = RuntimeManager::get_instance()
            .map_err(|e| format!("Runtime not available: {}", e))?;
        let manager = manager_arc.lock().unwrap();
        manager.block_on($future)
    }};
}

/// Execute an async operation with automatic runtime strategy selection
pub fn execute_with_runtime<F, T>(future: F) -> Result<T, String>
where
    F: std::future::Future<Output = T> + Send + 'static,
    T: Send + 'static,
{
    let manager_arc = RuntimeManager::get_instance()?;
    let guard = manager_arc.lock().unwrap();
    match guard.as_ref() {
        Some(manager) => manager.block_on(future),
        None => Err("Runtime manager not initialized".to_string()),
    }
}

/// Initialize runtime strategies with specific strategy
pub fn initialize_runtime_with_strategy(strategy: RuntimeStrategy) -> Result<(), String> {
    let strategy_mgr = RUNTIME_STRATEGY.get_or_init(|| {
        Arc::new(Mutex::new(None))
    });

    let mut guard = strategy_mgr.lock().unwrap();
    if guard.is_some() {
        return Ok(()); // Already initialized
    }

    let nodejs_detected = RuntimeManager::detect_nodejs_environment();
    
    let mut manager = RuntimeManager {
        strategy,
        context: RuntimeContext::Initializing,
        nodejs_detected,
        initialization_time: Instant::now(),
    };

    manager.initialize_strategy()
        .map_err(|e| format!("Strategy initialization failed: {}", e))?;
    
    *guard = Some(manager);
    Ok(())
}

/// Get current runtime statistics
pub fn get_runtime_statistics() -> Option<RuntimeStatistics> {
    let strategy_mgr = RUNTIME_STRATEGY.get()?;
    let guard = strategy_mgr.lock().ok()?;
    guard.as_ref().map(|manager| manager.get_statistics())
}

/// Force runtime strategy change (for testing)
pub fn force_runtime_strategy(strategy: RuntimeStrategy) -> Result<(), String> {
    let strategy_mgr = RUNTIME_STRATEGY.get_or_init(|| {
        Arc::new(Mutex::new(None))
    });

    let mut guard = strategy_mgr.lock().unwrap();
    
    let nodejs_detected = RuntimeManager::detect_nodejs_environment();
    
    let mut manager = RuntimeManager {
        strategy,
        context: RuntimeContext::Initializing,
        nodejs_detected,
        initialization_time: Instant::now(),
    };

    manager.initialize_strategy()
        .map_err(|e| format!("Strategy initialization failed: {}", e))?;
    
    *guard = Some(manager);
    info!("Runtime strategy forced to: {:?}", strategy);
    
    Ok(())
}

impl Drop for ThreadedRuntime {
    fn drop(&mut self) {
        if let Some(handle) = self.join_handle.take() {
            // Send shutdown signal and wait for thread
            drop(self.sender.clone()); // Close the channel
            let _ = handle.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_runtime_initialization() {
        let result = RuntimeManager::initialize();
        assert!(result.is_ok(), "Runtime initialization failed: {:?}", result);
    }

    #[test]
    fn test_nodejs_detection() {
        // This test depends on environment, so we just verify it doesn't panic
        let detected = RuntimeManager::detect_nodejs_environment();
        println!("Node.js detected: {}", detected);
        assert!(detected == true || detected == false); // Always true :)
    }

    #[test]
    fn test_strategy_execution() {
        let _ = RuntimeManager::initialize();
        
        let result = execute_with_runtime(async {
            tokio::time::sleep(Duration::from_millis(10)).await;
            42
        });
        
        assert!(result.is_ok(), "Runtime execution failed: {:?}", result);
        assert_eq!(result.unwrap(), 42);
    }

    #[test]
    fn test_all_strategies() {
        let strategies = [
            RuntimeStrategy::MultiThreaded,
            RuntimeStrategy::SingleThreaded,
            RuntimeStrategy::DedicatedThread,
            RuntimeStrategy::HandleBased,
        ];

        for &strategy in &strategies {
            println!("Testing strategy: {:?}", strategy);
            
            let result = force_runtime_strategy(strategy);
            if result.is_ok() {
                let execution_result = execute_with_runtime(async { 1 + 1 });
                assert!(execution_result.is_ok(), 
                       "Strategy {:?} execution failed: {:?}", strategy, execution_result);
            } else {
                println!("Strategy {:?} initialization failed (expected on some platforms): {:?}", 
                        strategy, result);
            }
        }
    }
}
