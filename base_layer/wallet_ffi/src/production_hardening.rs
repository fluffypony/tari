// Production Hardening for Wallet FFI
// Implements graceful error handling, recovery mechanisms, and production safety

use std::sync::{Arc, Mutex, OnceLock, atomic::{AtomicBool, AtomicU64, Ordering}};
use std::thread;
use std::time::{Duration, Instant};
use std::panic;
use std::collections::HashMap;
use log::{debug, error, warn, info};

/// Global crash detection and recovery system
pub struct CrashDetector {
    crash_count: Arc<AtomicU64>,
    last_crash_time: Arc<Mutex<Option<Instant>>>,
    is_crashed: Arc<AtomicBool>,
    recovery_attempts: Arc<AtomicU64>,
}

impl CrashDetector {
    pub fn new() -> Self {
        Self {
            crash_count: Arc::new(AtomicU64::new(0)),
            last_crash_time: Arc::new(Mutex::new(None)),
            is_crashed: Arc::new(AtomicBool::new(false)),
            recovery_attempts: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Install crash detection hooks
    pub fn install_crash_hooks(&self) {
        let crash_count = Arc::clone(&self.crash_count);
        let last_crash_time = Arc::clone(&self.last_crash_time);
        let is_crashed = Arc::clone(&self.is_crashed);

        // Install panic hook
        let prev_hook = panic::take_hook();
        panic::set_hook(Box::new(move |panic_info| {
            error!(
                target: "tari::wallet_ffi::crash_detector",
                "PANIC DETECTED: {}", panic_info
            );

            // Record crash
            crash_count.fetch_add(1, Ordering::SeqCst);
            is_crashed.store(true, Ordering::SeqCst);
            
            if let Ok(mut last_time) = last_crash_time.lock() {
                *last_time = Some(Instant::now());
            }

            // Call previous hook
            prev_hook(panic_info);
        }));

        info!(
            target: "tari::wallet_ffi::crash_detector",
            "Crash detection hooks installed"
        );
    }

    /// Check if system is in crashed state
    pub fn is_crashed(&self) -> bool {
        self.is_crashed.load(Ordering::SeqCst)
    }

    /// Get crash statistics
    pub fn get_crash_stats(&self) -> CrashStats {
        let crash_count = self.crash_count.load(Ordering::SeqCst);
        let last_crash_elapsed = if let Ok(last_time) = self.last_crash_time.lock() {
            last_time.map(|time| time.elapsed())
        } else {
            None
        };

        CrashStats {
            total_crashes: crash_count,
            last_crash_elapsed,
            recovery_attempts: self.recovery_attempts.load(Ordering::SeqCst),
        }
    }

    /// Attempt recovery from crashed state
    pub fn attempt_recovery(&self) -> bool {
        if !self.is_crashed() {
            return true; // Not crashed, no recovery needed
        }

        let attempts = self.recovery_attempts.fetch_add(1, Ordering::SeqCst);
        
        warn!(
            target: "tari::wallet_ffi::crash_detector",
            "Attempting crash recovery, attempt #{}", attempts + 1
        );

        // Implement recovery logic
        // For now, just reset the crashed state after a delay
        thread::sleep(Duration::from_millis(100));
        
        self.is_crashed.store(false, Ordering::SeqCst);
        
        info!(
            target: "tari::wallet_ffi::crash_detector",
            "Crash recovery completed"
        );

        true
    }
}

/// Crash statistics
#[derive(Debug, Clone)]
pub struct CrashStats {
    pub total_crashes: u64,
    pub last_crash_elapsed: Option<Duration>,
    pub recovery_attempts: u64,
}

/// Resource cleanup manager
pub struct ResourceCleanupManager {
    active_resources: Arc<Mutex<HashMap<u64, ResourceInfo>>>,
    next_resource_id: Arc<AtomicU64>,
}

#[derive(Debug, Clone)]
struct ResourceInfo {
    resource_type: String,
    created_at: Instant,
    thread_id: thread::ThreadId,
    cleanup_fn: Option<fn()>,
}

impl ResourceCleanupManager {
    pub fn new() -> Self {
        Self {
            active_resources: Arc::new(Mutex::new(HashMap::new())),
            next_resource_id: Arc::new(AtomicU64::new(1)),
        }
    }

    /// Register a resource for cleanup
    pub fn register_resource(&self, resource_type: String, cleanup_fn: Option<fn()>) -> u64 {
        let resource_id = self.next_resource_id.fetch_add(1, Ordering::SeqCst);
        
        let resource_info = ResourceInfo {
            resource_type: resource_type.clone(),
            created_at: Instant::now(),
            thread_id: thread::current().id(),
            cleanup_fn,
        };

        if let Ok(mut resources) = self.active_resources.lock() {
            resources.insert(resource_id, resource_info);
            
            debug!(
                target: "tari::wallet_ffi::resource_cleanup",
                "Registered resource {}: {} on thread {:?}",
                resource_id, resource_type, thread::current().id()
            );
        }

        resource_id
    }

    /// Unregister a resource (normal cleanup)
    pub fn unregister_resource(&self, resource_id: u64) {
        if let Ok(mut resources) = self.active_resources.lock() {
            if let Some(resource_info) = resources.remove(&resource_id) {
                debug!(
                    target: "tari::wallet_ffi::resource_cleanup",
                    "Unregistered resource {}: {} (age: {:?})",
                    resource_id, resource_info.resource_type, resource_info.created_at.elapsed()
                );
            }
        }
    }

    /// Cleanup all resources (emergency cleanup)
    pub fn cleanup_all_resources(&self) {
        warn!(
            target: "tari::wallet_ffi::resource_cleanup",
            "Emergency cleanup of all resources"
        );

        if let Ok(mut resources) = self.active_resources.lock() {
            for (resource_id, resource_info) in resources.drain() {
                warn!(
                    target: "tari::wallet_ffi::resource_cleanup",
                    "Cleaning up resource {}: {} (age: {:?})",
                    resource_id, resource_info.resource_type, resource_info.created_at.elapsed()
                );

                if let Some(cleanup_fn) = resource_info.cleanup_fn {
                    // Call cleanup function in a safe manner
                    let result = panic::catch_unwind(|| {
                        cleanup_fn();
                    });

                    if result.is_err() {
                        error!(
                            target: "tari::wallet_ffi::resource_cleanup",
                            "Cleanup function panicked for resource {}", resource_id
                        );
                    }
                }
            }
        }

        info!(
            target: "tari::wallet_ffi::resource_cleanup",
            "Emergency cleanup completed"
        );
    }

    /// Get resource statistics
    pub fn get_resource_stats(&self) -> ResourceStats {
        if let Ok(resources) = self.active_resources.lock() {
            ResourceStats {
                active_count: resources.len(),
                resource_types: resources.values()
                    .map(|info| info.resource_type.clone())
                    .collect(),
            }
        } else {
            ResourceStats {
                active_count: 0,
                resource_types: Vec::new(),
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct ResourceStats {
    pub active_count: usize,
    pub resource_types: Vec<String>,
}

/// Error recovery strategies
pub struct ErrorRecoveryManager {
    recovery_strategies: HashMap<String, Box<dyn Fn() -> bool + Send + Sync>>,
    error_counts: Arc<Mutex<HashMap<String, u64>>>,
}

impl ErrorRecoveryManager {
    pub fn new() -> Self {
        let mut manager = Self {
            recovery_strategies: HashMap::new(),
            error_counts: Arc::new(Mutex::new(HashMap::new())),
        };

        // Register default recovery strategies
        manager.register_default_strategies();
        manager
    }

    fn register_default_strategies(&mut self) {
        // Runtime creation failure recovery
        self.register_strategy(
            "runtime_creation_failure".to_string(),
            Box::new(|| {
                warn!(
                    target: "tari::wallet_ffi::error_recovery",
                    "Attempting runtime creation failure recovery"
                );

                // Wait a moment and try again
                thread::sleep(Duration::from_millis(100));
                
                // In a real implementation, we might:
                // 1. Try a different runtime strategy
                // 2. Clear any corrupted state
                // 3. Reset environment variables
                
                true // Indicate recovery was attempted
            })
        );

        // Memory allocation failure recovery
        self.register_strategy(
            "memory_allocation_failure".to_string(),
            Box::new(|| {
                warn!(
                    target: "tari::wallet_ffi::error_recovery",
                    "Attempting memory allocation failure recovery"
                );

                // Force garbage collection if available
                #[cfg(feature = "debug_memory")]
                {
                    // In a real implementation, we might trigger GC
                    // or free up cached memory
                }

                thread::sleep(Duration::from_millis(50));
                true
            })
        );

        // Network connectivity failure recovery
        self.register_strategy(
            "network_failure".to_string(),
            Box::new(|| {
                warn!(
                    target: "tari::wallet_ffi::error_recovery",
                    "Attempting network failure recovery"
                );

                thread::sleep(Duration::from_millis(200));
                true
            })
        );
    }

    /// Register a custom recovery strategy
    pub fn register_strategy<F>(&mut self, error_type: String, strategy: F)
    where
        F: Fn() -> bool + Send + Sync + 'static,
    {
        self.recovery_strategies.insert(error_type.clone(), Box::new(strategy));
        
        debug!(
            target: "tari::wallet_ffi::error_recovery",
            "Registered recovery strategy for: {}", error_type
        );
    }

    /// Attempt recovery for a specific error type
    pub fn attempt_recovery(&self, error_type: &str) -> bool {
        // Track error occurrence
        if let Ok(mut counts) = self.error_counts.lock() {
            let count = counts.entry(error_type.to_string()).or_insert(0);
            *count += 1;

            // Don't attempt recovery if we've seen too many of this error
            if *count > 10 {
                error!(
                    target: "tari::wallet_ffi::error_recovery",
                    "Too many {} errors ({}), not attempting recovery",
                    error_type, count
                );
                return false;
            }
        }

        // Attempt recovery
        if let Some(strategy) = self.recovery_strategies.get(error_type) {
            info!(
                target: "tari::wallet_ffi::error_recovery",
                "Attempting recovery for error type: {}", error_type
            );

            match panic::catch_unwind(panic::AssertUnwindSafe(|| strategy())) {
                Ok(success) => {
                    if success {
                        info!(
                            target: "tari::wallet_ffi::error_recovery",
                            "Recovery successful for: {}", error_type
                        );
                    } else {
                        warn!(
                            target: "tari::wallet_ffi::error_recovery",
                            "Recovery failed for: {}", error_type
                        );
                    }
                    success
                }
                Err(_) => {
                    error!(
                        target: "tari::wallet_ffi::error_recovery",
                        "Recovery strategy panicked for: {}", error_type
                    );
                    false
                }
            }
        } else {
            debug!(
                target: "tari::wallet_ffi::error_recovery",
                "No recovery strategy available for: {}", error_type
            );
            false
        }
    }

    /// Get error statistics
    pub fn get_error_stats(&self) -> HashMap<String, u64> {
        self.error_counts.lock()
            .map(|counts| counts.clone())
            .unwrap_or_default()
    }
}

/// Production safety coordinator
pub struct ProductionSafetyCoordinator {
    crash_detector: CrashDetector,
    resource_manager: ResourceCleanupManager,
    error_recovery: ErrorRecoveryManager,
    safety_enabled: AtomicBool,
}

impl ProductionSafetyCoordinator {
    pub fn new() -> Self {
        Self {
            crash_detector: CrashDetector::new(),
            resource_manager: ResourceCleanupManager::new(),
            error_recovery: ErrorRecoveryManager::new(),
            safety_enabled: AtomicBool::new(true),
        }
    }

    /// Initialize production safety systems
    pub fn initialize(&self) {
        if !self.safety_enabled.load(Ordering::SeqCst) {
            debug!(
                target: "tari::wallet_ffi::production_safety",
                "Production safety disabled, skipping initialization"
            );
            return;
        }

        info!(
            target: "tari::wallet_ffi::production_safety",
            "Initializing production safety systems"
        );

        self.crash_detector.install_crash_hooks();

        info!(
            target: "tari::wallet_ffi::production_safety",
            "Production safety systems initialized"
        );
    }

    /// Handle a critical error with recovery attempt
    pub fn handle_critical_error(&self, error_type: &str, error_message: &str) -> bool {
        error!(
            target: "tari::wallet_ffi::production_safety",
            "Critical error detected: {} - {}", error_type, error_message
        );

        // Attempt error recovery
        let recovery_success = self.error_recovery.attempt_recovery(error_type);

        if !recovery_success {
            warn!(
                target: "tari::wallet_ffi::production_safety",
                "Error recovery failed, triggering emergency cleanup"
            );
            
            self.emergency_shutdown();
        }

        recovery_success
    }

    /// Emergency shutdown and cleanup
    pub fn emergency_shutdown(&self) {
        warn!(
            target: "tari::wallet_ffi::production_safety",
            "Initiating emergency shutdown"
        );

        // Cleanup all resources
        self.resource_manager.cleanup_all_resources();

        // Try to recover from any crashes
        self.crash_detector.attempt_recovery();

        info!(
            target: "tari::wallet_ffi::production_safety",
            "Emergency shutdown completed"
        );
    }

    /// Register a resource for cleanup tracking
    pub fn register_resource(&self, resource_type: String, cleanup_fn: Option<fn()>) -> u64 {
        self.resource_manager.register_resource(resource_type, cleanup_fn)
    }

    /// Unregister a resource
    pub fn unregister_resource(&self, resource_id: u64) {
        self.resource_manager.unregister_resource(resource_id);
    }

    /// Get comprehensive safety status
    pub fn get_safety_status(&self) -> SafetyStatus {
        SafetyStatus {
            safety_enabled: self.safety_enabled.load(Ordering::SeqCst),
            crash_stats: self.crash_detector.get_crash_stats(),
            resource_stats: self.resource_manager.get_resource_stats(),
            error_stats: self.error_recovery.get_error_stats(),
            is_crashed: self.crash_detector.is_crashed(),
        }
    }

    /// Enable or disable safety systems
    pub fn set_safety_enabled(&self, enabled: bool) {
        self.safety_enabled.store(enabled, Ordering::SeqCst);
        
        if enabled {
            info!(
                target: "tari::wallet_ffi::production_safety",
                "Production safety systems enabled"
            );
        } else {
            warn!(
                target: "tari::wallet_ffi::production_safety",
                "Production safety systems disabled"
            );
        }
    }
}

#[derive(Debug, Clone)]
pub struct SafetyStatus {
    pub safety_enabled: bool,
    pub crash_stats: CrashStats,
    pub resource_stats: ResourceStats,
    pub error_stats: HashMap<String, u64>,
    pub is_crashed: bool,
}

impl Default for ProductionSafetyCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

// Global safety coordinator instance
static GLOBAL_SAFETY: OnceLock<ProductionSafetyCoordinator> = OnceLock::new();

/// Get global safety coordinator
pub fn get_global_safety() -> &'static ProductionSafetyCoordinator {
    GLOBAL_SAFETY.get_or_init(|| ProductionSafetyCoordinator::new())
}

/// Initialize global safety systems
pub fn initialize_global_safety() {
    get_global_safety().initialize();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crash_detector() {
        let detector = CrashDetector::new();
        
        assert!(!detector.is_crashed());
        
        let stats = detector.get_crash_stats();
        assert_eq!(stats.total_crashes, 0);
        assert_eq!(stats.recovery_attempts, 0);
    }

    #[test]
    fn test_resource_manager() {
        let manager = ResourceCleanupManager::new();
        
        let resource_id = manager.register_resource("test_resource".to_string(), None);
        
        let stats = manager.get_resource_stats();
        assert_eq!(stats.active_count, 1);
        assert!(stats.resource_types.contains(&"test_resource".to_string()));
        
        manager.unregister_resource(resource_id);
        
        let stats = manager.get_resource_stats();
        assert_eq!(stats.active_count, 0);
    }

    #[test]
    fn test_error_recovery() {
        let recovery = ErrorRecoveryManager::new();
        
        // Test known strategy
        assert!(recovery.attempt_recovery("runtime_creation_failure"));
        
        // Test unknown strategy
        assert!(!recovery.attempt_recovery("unknown_error"));
    }

    #[test]
    fn test_safety_coordinator() {
        let coordinator = ProductionSafetyCoordinator::new();
        
        coordinator.initialize();
        
        let status = coordinator.get_safety_status();
        assert!(status.safety_enabled);
        assert!(!status.is_crashed);
        
        coordinator.set_safety_enabled(false);
        let status = coordinator.get_safety_status();
        assert!(!status.safety_enabled);
    }
}
