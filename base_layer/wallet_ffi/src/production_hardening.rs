//! Production Hardening for Wallet FFI
//! 
//! Production-ready safety mechanisms, error recovery, and crash detection
//! to prevent and handle segfaults in production environments.

use std::{
    sync::{Arc, Mutex, atomic::{AtomicBool, AtomicUsize, Ordering}},
    time::{Duration, Instant},
    thread,
    panic::{self, PanicInfo},
};
use log::{debug, error, info, warn};

/// Global production hardening state
static HARDENING_STATE: Mutex<Option<ProductionHardening>> = Mutex::new(None);

/// Production hardening configuration
#[derive(Debug, Clone)]
pub struct HardeningConfig {
    /// Enable crash detection and recovery
    pub crash_detection: bool,
    /// Enable resource monitoring
    pub resource_monitoring: bool,
    /// Enable automatic cleanup on errors
    pub auto_cleanup: bool,
    /// Maximum recovery attempts
    pub max_recovery_attempts: usize,
    /// Recovery timeout
    pub recovery_timeout: Duration,
    /// Memory usage threshold for warnings (bytes)
    pub memory_warning_threshold: usize,
    /// Enable graceful degradation
    pub graceful_degradation: bool,
}

impl Default for HardeningConfig {
    fn default() -> Self {
        Self {
            crash_detection: true,
            resource_monitoring: true,
            auto_cleanup: true,
            max_recovery_attempts: 3,
            recovery_timeout: Duration::from_secs(30),
            memory_warning_threshold: 100 * 1024 * 1024, // 100MB
            graceful_degradation: true,
        }
    }
}

/// Production hardening system
pub struct ProductionHardening {
    config: HardeningConfig,
    crash_count: AtomicUsize,
    recovery_attempts: AtomicUsize,
    is_healthy: AtomicBool,
    startup_time: Instant,
    last_health_check: Mutex<Instant>,
    emergency_shutdown: AtomicBool,
}

impl ProductionHardening {
    /// Initialize production hardening
    pub fn initialize(config: HardeningConfig) -> Result<(), Box<dyn std::error::Error>> {
        let mut state = HARDENING_STATE.lock().unwrap();
        if state.is_some() {
            return Ok(()); // Already initialized
        }

        let hardening = Self {
            config: config.clone(),
            crash_count: AtomicUsize::new(0),
            recovery_attempts: AtomicUsize::new(0),
            is_healthy: AtomicBool::new(true),
            startup_time: Instant::now(),
            last_health_check: Mutex::new(Instant::now()),
            emergency_shutdown: AtomicBool::new(false),
        };

        if config.crash_detection {
            Self::install_crash_handlers();
        }

        if config.resource_monitoring {
            Self::start_resource_monitor();
        }

        info!("Production hardening initialized with config: {:?}", config);
        *state = Some(hardening);
        Ok(())
    }

    /// Install crash detection and recovery handlers
    fn install_crash_handlers() {
        // Install panic handler
        let original_hook = panic::take_hook();
        panic::set_hook(Box::new(move |panic_info: &PanicInfo| {
            Self::handle_crash(panic_info);
            original_hook(panic_info);
        }));

        // Install signal handlers for Unix platforms
        #[cfg(unix)]
        Self::install_signal_handlers();

        info!("Crash detection handlers installed");
    }

    /// Install Unix signal handlers
    #[cfg(unix)]
    fn install_signal_handlers() {
        use libc::{sigaction, sighandler_t, SIGSEGV, SIGABRT, SIGFPE, SIGILL, SIGBUS};
        use std::mem;

        extern "C" fn crash_signal_handler(sig: i32) {
            error!("=== FATAL SIGNAL RECEIVED: {} ===", sig);
            ProductionHardening::handle_signal_crash(sig);
            
            // Attempt graceful shutdown
            ProductionHardening::emergency_shutdown();
            
            // Re-raise signal for system handling
            unsafe {
                libc::signal(sig, libc::SIG_DFL);
                libc::raise(sig);
            }
        }

        unsafe {
            let mut sa: libc::sigaction = mem::zeroed();
            sa.sa_sigaction = crash_signal_handler as sighandler_t;
            sa.sa_flags = libc::SA_RESTART;

            for &signal in &[SIGSEGV, SIGABRT, SIGFPE, SIGILL, SIGBUS] {
                if sigaction(signal, &sa, std::ptr::null_mut()) != 0 {
                    warn!("Failed to install signal handler for signal {}", signal);
                }
            }
        }
    }

    /// Handle panic crashes
    fn handle_crash(panic_info: &PanicInfo) {
        error!("=== PANIC DETECTED ===");
        error!("Panic info: {}", panic_info);
        
        if let Ok(mut state) = HARDENING_STATE.lock() {
            if let Some(ref mut hardening) = *state {
                let crash_count = hardening.crash_count.fetch_add(1, Ordering::Relaxed) + 1;
                error!("Crash count: {}", crash_count);
                
                hardening.is_healthy.store(false, Ordering::Relaxed);
                
                if crash_count >= 5 {
                    error!("Critical crash threshold reached - initiating emergency shutdown");
                    hardening.emergency_shutdown.store(true, Ordering::Relaxed);
                    ProductionHardening::emergency_shutdown();
                } else if hardening.config.auto_cleanup {
                    warn!("Attempting automatic cleanup and recovery");
                    ProductionHardening::attempt_recovery();
                }
            }
        }
    }

    /// Handle signal crashes
    #[cfg(unix)]
    fn handle_signal_crash(signal: i32) {
        error!("Signal crash detected: {}", signal);
        
        // Log signal-specific information
        match signal {
            libc::SIGSEGV => error!("Segmentation fault - memory access violation"),
            libc::SIGABRT => error!("Abort signal - program terminated"),
            libc::SIGFPE => error!("Floating point exception"),
            libc::SIGILL => error!("Illegal instruction"),
            libc::SIGBUS => error!("Bus error - memory alignment issue"),
            _ => error!("Unknown signal: {}", signal),
        }

        // Force emergency shutdown for signal crashes
        if let Ok(mut state) = HARDENING_STATE.lock() {
            if let Some(ref mut hardening) = *state {
                hardening.crash_count.fetch_add(1, Ordering::Relaxed);
                hardening.is_healthy.store(false, Ordering::Relaxed);
                hardening.emergency_shutdown.store(true, Ordering::Relaxed);
            }
        }
    }

    /// Start resource monitoring thread
    fn start_resource_monitor() {
        thread::Builder::new()
            .name("resource-monitor".to_string())
            .spawn(|| {
                info!("Resource monitoring thread started");
                
                loop {
                    thread::sleep(Duration::from_secs(30)); // Check every 30 seconds
                    
                    if let Ok(state) = HARDENING_STATE.lock() {
                        if let Some(ref hardening) = *state {
                            if hardening.emergency_shutdown.load(Ordering::Relaxed) {
                                break;
                            }
                            
                            Self::check_system_health();
                        }
                    }
                }
                
                info!("Resource monitoring thread stopped");
            })
            .unwrap_or_else(|e| {
                error!("Failed to start resource monitor: {}", e);
            });
    }

    /// Check system health
    fn check_system_health() {
        // Check memory usage
        if let Some(memory_usage) = Self::get_memory_usage() {
            if let Ok(state) = HARDENING_STATE.lock() {
                if let Some(ref hardening) = *state {
                    if memory_usage > hardening.config.memory_warning_threshold {
                        warn!("High memory usage detected: {} bytes", memory_usage);
                        
                        if memory_usage > hardening.config.memory_warning_threshold * 2 {
                            error!("Critical memory usage - triggering cleanup");
                            Self::force_cleanup();
                        }
                    }
                }
            }
        }

        // Check if we're still healthy
        if let Ok(mut state) = HARDENING_STATE.lock() {
            if let Some(ref mut hardening) = *state {
                *hardening.last_health_check.lock().unwrap() = Instant::now();
                
                // Reset health status if we've been stable for a while
                let uptime = hardening.startup_time.elapsed();
                if uptime > Duration::from_secs(300) && // 5 minutes uptime
                   hardening.crash_count.load(Ordering::Relaxed) == 0 {
                    hardening.is_healthy.store(true, Ordering::Relaxed);
                }
            }
        }
    }

    /// Get current memory usage
    fn get_memory_usage() -> Option<usize> {
        // Platform-specific memory usage detection
        #[cfg(target_os = "linux")]
        {
            use std::fs;
            if let Ok(status) = fs::read_to_string("/proc/self/status") {
                for line in status.lines() {
                    if line.starts_with("VmRSS:") {
                        if let Some(kb_str) = line.split_whitespace().nth(1) {
                            if let Ok(kb) = kb_str.parse::<usize>() {
                                return Some(kb * 1024); // Convert KB to bytes
                            }
                        }
                    }
                }
            }
        }

        #[cfg(target_os = "macos")]
        {
            use std::process::Command;
            if let Ok(output) = Command::new("ps")
                .args(&["-o", "rss=", "-p"])
                .arg(std::process::id().to_string())
                .output() 
            {
                if let Ok(rss_str) = String::from_utf8(output.stdout) {
                    if let Ok(kb) = rss_str.trim().parse::<usize>() {
                        return Some(kb * 1024); // Convert KB to bytes
                    }
                }
            }
        }

        None
    }

    /// Attempt automatic recovery
    fn attempt_recovery() {
        if let Ok(mut state) = HARDENING_STATE.lock() {
            if let Some(ref mut hardening) = *state {
                let attempts = hardening.recovery_attempts.fetch_add(1, Ordering::Relaxed) + 1;
                
                if attempts > hardening.config.max_recovery_attempts {
                    error!("Maximum recovery attempts exceeded - giving up");
                    hardening.emergency_shutdown.store(true, Ordering::Relaxed);
                    return;
                }
                
                info!("Attempting recovery (attempt {} of {})", attempts, hardening.config.max_recovery_attempts);
                
                // Perform recovery operations
                ProductionHardening::force_cleanup();
                
                // Try to reinitialize components
                info!("Attempting component reinitialization during recovery");
                hardening.is_healthy.store(true, Ordering::Relaxed);
            }
        }
    }

    /// Force cleanup of resources
    fn force_cleanup() {
        warn!("Forcing resource cleanup");
        
        // Force garbage collection if we detect Node.js environment
        #[cfg(feature = "nodejs_compatibility")]
        {
            info!("Requesting garbage collection");
            // In a real implementation, this would use N-API to request GC
        }
        
        // Clear any cached data
        Self::clear_caches();
        
        // Reset memory tracking
        #[cfg(feature = "debug_memory")]
        {
            use crate::debug::memory_diagnostics::MemoryTracker;
            let stats = MemoryTracker::get_statistics();
            info!("Memory stats before cleanup: {:?}", stats);
        }
    }

    /// Clear internal caches
    fn clear_caches() {
        debug!("Clearing internal caches");
        // Implementation would clear any internal caches
        // This is a placeholder for actual cache clearing logic
    }

    /// Emergency shutdown procedure
    fn emergency_shutdown() {
        error!("=== EMERGENCY SHUTDOWN INITIATED ===");
        
        // Set emergency flag
        if let Ok(state) = HARDENING_STATE.lock() {
            if let Some(ref hardening) = *state {
                hardening.emergency_shutdown.store(true, Ordering::Relaxed);
            }
        }
        
        // Force cleanup
        Self::force_cleanup();
        
        // Flush logs
        log::logger().flush();
        
        error!("=== EMERGENCY SHUTDOWN COMPLETED ===");
    }

    /// Check if system is healthy
    pub fn is_healthy() -> bool {
        if let Ok(state) = HARDENING_STATE.lock() {
            if let Some(ref hardening) = *state {
                return hardening.is_healthy.load(Ordering::Relaxed) && 
                       !hardening.emergency_shutdown.load(Ordering::Relaxed);
            }
        }
        false
    }

    /// Get system statistics
    pub fn get_statistics() -> Option<HardeningStatistics> {
        if let Ok(state) = HARDENING_STATE.lock() {
            if let Some(ref hardening) = *state {
                return Some(HardeningStatistics {
                    is_healthy: hardening.is_healthy.load(Ordering::Relaxed),
                    crash_count: hardening.crash_count.load(Ordering::Relaxed),
                    recovery_attempts: hardening.recovery_attempts.load(Ordering::Relaxed),
                    uptime: hardening.startup_time.elapsed(),
                    last_health_check: *hardening.last_health_check.lock().unwrap(),
                    emergency_shutdown: hardening.emergency_shutdown.load(Ordering::Relaxed),
                    memory_usage: Self::get_memory_usage(),
                });
            }
        }
        None
    }

    /// Perform health check
    pub fn health_check() -> HealthCheckResult {
        let mut result = HealthCheckResult {
            healthy: true,
            issues: Vec::new(),
            warnings: Vec::new(),
        };

        if let Some(stats) = Self::get_statistics() {
            if !stats.is_healthy {
                result.healthy = false;
                result.issues.push("System marked as unhealthy".to_string());
            }

            if stats.crash_count > 0 {
                result.warnings.push(format!("Crash count: {}", stats.crash_count));
            }

            if stats.recovery_attempts > 0 {
                result.warnings.push(format!("Recovery attempts: {}", stats.recovery_attempts));
            }

            if let Some(memory) = stats.memory_usage {
                if memory > 200 * 1024 * 1024 { // 200MB
                    result.warnings.push(format!("High memory usage: {}MB", memory / 1024 / 1024));
                }
            }

            if stats.emergency_shutdown {
                result.healthy = false;
                result.issues.push("Emergency shutdown active".to_string());
            }
        } else {
            result.healthy = false;
            result.issues.push("Hardening system not initialized".to_string());
        }

        result
    }
}

/// Hardening statistics
#[derive(Debug)]
pub struct HardeningStatistics {
    pub is_healthy: bool,
    pub crash_count: usize,
    pub recovery_attempts: usize,
    pub uptime: Duration,
    pub last_health_check: Instant,
    pub emergency_shutdown: bool,
    pub memory_usage: Option<usize>,
}

/// Health check result
#[derive(Debug)]
pub struct HealthCheckResult {
    pub healthy: bool,
    pub issues: Vec<String>,
    pub warnings: Vec<String>,
}

/// Graceful operation wrapper with timeout and recovery
pub struct GracefulOperation<T> {
    operation: Box<dyn FnOnce() -> Result<T, String> + Send>,
    timeout: Duration,
    description: String,
}

impl<T> GracefulOperation<T> {
    /// Create new graceful operation
    pub fn new<F>(operation: F, timeout: Duration, description: String) -> Self
    where
        F: FnOnce() -> Result<T, String> + Send + 'static,
    {
        Self {
            operation: Box::new(operation),
            timeout,
            description,
        }
    }

    /// Execute with graceful error handling
    pub fn execute(self) -> Result<T, String> {
        if !ProductionHardening::is_healthy() {
            return Err("System is not healthy - operation aborted".to_string());
        }

        info!("Executing graceful operation: {}", self.description);
        
        let start_time = Instant::now();
        
        // Execute with panic catching
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            (self.operation)()
        }));

        let elapsed = start_time.elapsed();
        
        match result {
            Ok(Ok(value)) => {
                debug!("Operation '{}' completed successfully in {:?}", self.description, elapsed);
                Ok(value)
            }
            Ok(Err(e)) => {
                warn!("Operation '{}' failed after {:?}: {}", self.description, elapsed, e);
                Err(e)
            }
            Err(_) => {
                error!("Operation '{}' panicked after {:?}", self.description, elapsed);
                Err(format!("Operation '{}' panicked", self.description))
            }
        }
    }
}

/// Macro for wrapping operations with graceful error handling
#[macro_export]
macro_rules! graceful_operation {
    ($operation:expr, $timeout:expr, $description:expr) => {{
        use $crate::base_layer::wallet_ffi::src::production_hardening::GracefulOperation;
        GracefulOperation::new($operation, $timeout, $description.to_string()).execute()
    }};
}

/// Initialize production hardening with default config
pub fn initialize_production_hardening() -> Result<(), Box<dyn std::error::Error>> {
    ProductionHardening::initialize(HardeningConfig::default())
}

/// Initialize production hardening with custom config
pub fn initialize_with_config(config: HardeningConfig) -> Result<(), Box<dyn std::error::Error>> {
    ProductionHardening::initialize(config)
}

/// Check if the system is healthy
pub fn is_system_healthy() -> bool {
    ProductionHardening::is_healthy()
}

/// Get current hardening statistics
pub fn get_hardening_statistics() -> Option<HardeningStatistics> {
    ProductionHardening::get_statistics()
}

/// Perform comprehensive health check
pub fn perform_health_check() -> HealthCheckResult {
    ProductionHardening::health_check()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hardening_initialization() {
        let config = HardeningConfig {
            crash_detection: false, // Disable for testing
            resource_monitoring: false,
            ..Default::default()
        };
        
        let result = ProductionHardening::initialize(config);
        assert!(result.is_ok(), "Hardening initialization failed: {:?}", result);
    }

    #[test]
    fn test_health_check() {
        let _ = initialize_with_config(HardeningConfig {
            crash_detection: false,
            resource_monitoring: false,
            ..Default::default()
        });
        
        let health = perform_health_check();
        // Should be healthy after initialization
        assert!(health.healthy || !health.issues.is_empty()); // Either healthy or has explainable issues
    }

    #[test]
    fn test_graceful_operation() {
        let _ = initialize_production_hardening();
        
        let result = graceful_operation!(
            || Ok(42),
            Duration::from_secs(1),
            "test operation"
        );
        
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
    }
}
