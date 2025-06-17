//! Segfault Investigation Toolkit
//! 
//! Comprehensive debugging utilities for investigating segfaults in the Tari wallet FFI,
//! particularly those caused by Tokio runtime conflicts with Node.js event loops.

use std::{
    backtrace::{Backtrace, BacktraceStatus},
    panic::{self, PanicHookInfo},
    sync::Mutex,
    thread,
    time::Instant,
};
use log::{error, info, warn};

#[cfg(feature = "debug_memory")]
use std::alloc::{GlobalAlloc, Layout, System};

/// Global segfault investigation state
static INVESTIGATION_STATE: Mutex<Option<SegfaultInvestigator>> = Mutex::new(None);

/// Comprehensive segfault investigation toolkit
pub struct SegfaultInvestigator {
    /// Start time for timing analysis
    start_time: Instant,
    /// Panic handler installed
    panic_handler_installed: bool,
    /// Thread analysis enabled
    thread_analysis: bool,
    /// Memory tracking enabled
    memory_tracking: bool,
}

impl SegfaultInvestigator {
    /// Initialize segfault investigation with all debugging features
    pub fn initialize() -> Result<(), Box<dyn std::error::Error>> {
        let mut state = INVESTIGATION_STATE.lock().unwrap();
        if state.is_some() {
            return Ok(()); // Already initialized
        }

        let investigator = Self {
            start_time: Instant::now(),
            panic_handler_installed: false,
            thread_analysis: true,
            memory_tracking: cfg!(feature = "debug_memory"),
        };

        // Install panic handler
        Self::install_panic_handler();
        
        // Setup signal handlers for segfaults (Unix only)
        #[cfg(unix)]
        Self::setup_signal_handlers()?;

        info!("Segfault investigation initialized");
        *state = Some(investigator);
        Ok(())
    }

    /// Install comprehensive panic handler with backtrace
    fn install_panic_handler() {
        panic::set_hook(Box::new(|panic_info: &PanicHookInfo| {
            let backtrace = Backtrace::capture();
            let thread = thread::current();
            let thread_name = thread.name().unwrap_or("unnamed");
            
            error!("=== PANIC DETECTED ===");
            error!("Thread: {}", thread_name);
            error!("Panic info: {}", panic_info);
            
            if backtrace.status() == BacktraceStatus::Captured {
                error!("Backtrace:\n{}", backtrace);
            } else {
                error!("Backtrace not available (set RUST_BACKTRACE=1)");
            }
            
            // Log Node.js context if available
            SegfaultInvestigator::log_nodejs_context();
            
            // Log Tokio runtime state
            SegfaultInvestigator::log_tokio_runtime_state();
            
            error!("=== END PANIC REPORT ===");
        }));
    }

    /// Setup signal handlers for segfaults (Unix platforms)
    #[cfg(unix)]
    fn setup_signal_handlers() -> Result<(), Box<dyn std::error::Error>> {
        use libc::{sigaction, sighandler_t, SIGSEGV, SIGABRT, SIGFPE, SIGILL};
        use std::mem;
        
        extern "C" fn segfault_handler(sig: i32) {
            error!("=== SEGMENTATION FAULT DETECTED ===");
            error!("Signal: {}", sig);
            
            let backtrace = Backtrace::force_capture();
            error!("Backtrace:\n{}", backtrace);
            
            SegfaultInvestigator::log_nodejs_context();
            SegfaultInvestigator::log_tokio_runtime_state();
            SegfaultInvestigator::emergency_cleanup();
            
            error!("=== END SEGFAULT REPORT ===");
            
            // Re-raise signal for system handling
            unsafe {
                libc::signal(sig, libc::SIG_DFL);
                libc::raise(sig);
            }
        }

        unsafe {
            let mut sa: libc::sigaction = mem::zeroed();
            sa.sa_sigaction = segfault_handler as sighandler_t;
            sa.sa_flags = libc::SA_RESTART;

            for &signal in &[SIGSEGV, SIGABRT, SIGFPE, SIGILL] {
                if sigaction(signal, &sa, std::ptr::null_mut()) != 0 {
                    return Err("Failed to install signal handler".into());
                }
            }
        }

        info!("Signal handlers installed for segfault detection");
        Ok(())
    }

    /// Log Node.js context information
    fn log_nodejs_context() {
        error!("=== NODE.JS CONTEXT ===");
        
        // Check if we're in a Node.js environment
        if let Ok(node_version) = std::env::var("NODE_VERSION") {
            error!("Node.js version: {}", node_version);
        }
        
        // Check for UV event loop
        error!("Thread ID: {:?}", thread::current().id());
        error!("Process ID: {}", std::process::id());
        
        // Check environment variables that indicate Node.js
        for var in &["NODE_ENV", "npm_config_registry", "npm_lifecycle_event"] {
            if let Ok(value) = std::env::var(var) {
                error!("{}: {}", var, value);
            }
        }
    }

    /// Log Tokio runtime state
    fn log_tokio_runtime_state() {
        error!("=== TOKIO RUNTIME STATE ===");
        
        // Check if we're in a Tokio context
        if tokio::runtime::Handle::try_current().is_ok() {
            error!("Tokio runtime handle is available");
            
            // Log runtime metrics if available
            #[cfg(feature = "tokio-metrics")]
            {
                if let Ok(handle) = tokio::runtime::Handle::try_current() {
                    let metrics = handle.metrics();
                    error!("Active tasks: {}", metrics.active_tasks_count());
                    error!("Blocking threads: {}", metrics.num_blocking_threads());
                    error!("Idle blocking threads: {}", metrics.num_idle_blocking_threads());
                }
            }
        } else {
            error!("No Tokio runtime handle available");
        }
        
        // Thread information
        error!("Current thread: {:?}", thread::current().name());
        error!("Is main thread: {}", thread::current().name() == Some("main"));
    }

    /// Emergency cleanup procedures
    fn emergency_cleanup() {
        warn!("Performing emergency cleanup...");
        
        // Attempt to flush logs
        log::logger().flush();
        
        // Additional cleanup can be added here
        warn!("Emergency cleanup completed");
    }

    /// Record a critical operation for debugging
    pub fn record_operation(operation: &str) {
        if let Ok(state) = INVESTIGATION_STATE.lock() {
            if state.is_some() {
                info!("DEBUG_OP: {} at {:?}", operation, Instant::now());
            }
        }
    }

    /// Record Node.js specific operation
    pub fn record_nodejs_operation(operation: &str, details: &str) {
        Self::record_operation(&format!("NODEJS: {} - {}", operation, details));
    }

    /// Record Tokio runtime operation
    pub fn record_tokio_operation(operation: &str, details: &str) {
        Self::record_operation(&format!("TOKIO: {} - {}", operation, details));
    }
}

/// Memory tracking allocator for debugging
#[cfg(feature = "debug_memory")]
pub struct TrackingAllocator;

#[cfg(feature = "debug_memory")]
unsafe impl GlobalAlloc for TrackingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = System.alloc(layout);
        if !ptr.is_null() {
            SegfaultInvestigator::record_operation(
                &format!("ALLOC: {} bytes at {:?}", layout.size(), ptr)
            );
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        SegfaultInvestigator::record_operation(
            &format!("DEALLOC: {} bytes at {:?}", layout.size(), ptr)
        );
        System.dealloc(ptr, layout);
    }
}

#[cfg(feature = "debug_memory")]
#[global_allocator]
static GLOBAL: TrackingAllocator = TrackingAllocator;

/// Macro for easy debugging of critical sections
#[macro_export]
macro_rules! debug_critical_section {
    ($operation:expr, $code:block) => {{
        SegfaultInvestigator::record_operation(&format!("ENTER: {}", $operation));
        let result = $code;
        SegfaultInvestigator::record_operation(&format!("EXIT: {}", $operation));
        result
    }};
}

/// Macro for debugging Node.js FFI boundaries
#[macro_export]
macro_rules! debug_nodejs_ffi {
    ($operation:expr, $code:block) => {{
        SegfaultInvestigator::record_nodejs_operation($operation, "FFI boundary entry");
        let result = $code;
        SegfaultInvestigator::record_nodejs_operation($operation, "FFI boundary exit");
        result
    }};
}

/// Initialize debugging for tests
pub fn init_test_debugging() {
    #[cfg(feature = "env_logger")]
    let _ = env_logger::try_init();
    let _ = SegfaultInvestigator::initialize();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_investigation_initialization() {
        init_test_debugging();
        // Test should not panic
        assert!(true);
    }

    #[test]
    fn test_operation_recording() {
        init_test_debugging();
        SegfaultInvestigator::record_operation("test_operation");
        SegfaultInvestigator::record_nodejs_operation("test_nodejs", "test_details");
        SegfaultInvestigator::record_tokio_operation("test_tokio", "test_details");
    }
}
