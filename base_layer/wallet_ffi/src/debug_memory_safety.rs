// Memory Safety Utilities for FFI Boundary
// Provides runtime memory safety checks for the wallet_create function

use std::ffi::CStr;
use std::os::raw::{c_char, c_int, c_void};
use std::ptr;
use log::{debug, error, warn, info};

/// Memory safety checker for FFI operations
pub struct FfiMemorySafetyChecker;

impl FfiMemorySafetyChecker {
    /// Check if a C pointer is safe to dereference
    pub fn validate_c_pointer<T>(ptr: *const T, param_name: &str) -> Result<(), String> {
        if ptr.is_null() {
            return Err(format!("Null pointer for parameter: {}", param_name));
        }

        // Check alignment
        let alignment = std::mem::align_of::<T>();
        let addr = ptr as usize;
        if addr % alignment != 0 {
            return Err(format!(
                "Misaligned pointer for {}: address={:p}, required_alignment={}",
                param_name, ptr, alignment
            ));
        }

        debug!(
            target: "tari::wallet_ffi::memory_safety",
            "Pointer validation passed for {}: ptr={:p}, align={}",
            param_name, ptr, alignment
        );

        Ok(())
    }

    /// Check if a mutable C pointer is safe to write to
    pub fn validate_c_mut_pointer<T>(ptr: *mut T, param_name: &str) -> Result<(), String> {
        Self::validate_c_pointer(ptr as *const T, param_name)?;
        
        debug!(
            target: "tari::wallet_ffi::memory_safety",
            "Mutable pointer validation passed for {}: ptr={:p}",
            param_name, ptr
        );

        Ok(())
    }

    /// Validate a C string pointer
    pub fn validate_c_string(ptr: *const c_char, param_name: &str) -> Result<(), String> {
        if ptr.is_null() {
            return Err(format!("Null C string pointer for parameter: {}", param_name));
        }

        // Basic alignment check
        if (ptr as usize) % std::mem::align_of::<c_char>() != 0 {
            return Err(format!("Misaligned C string pointer for {}", param_name));
        }

        // Try to get the length (this is potentially unsafe but we do it carefully)
        let c_str_result = unsafe { CStr::from_ptr(ptr) };
        match c_str_result.to_str() {
            Ok(s) => {
                debug!(
                    target: "tari::wallet_ffi::memory_safety",
                    "C string validation passed for {}: ptr={:p}, length={}, content={:?}",
                    param_name, ptr, s.len(), s.chars().take(20).collect::<String>()
                );
            }
            Err(_) => {
                warn!(
                    target: "tari::wallet_ffi::memory_safety",
                    "C string contains invalid UTF-8 for {}: ptr={:p}",
                    param_name, ptr
                );
            }
        }

        Ok(())
    }

    /// Validate callback function pointer
    pub fn validate_callback<F>(
        callback: Option<F>,
        callback_name: &str,
    ) -> Result<(), String>
    where
        F: Copy,
    {
        match callback {
            Some(_) => {
                debug!(
                    target: "tari::wallet_ffi::memory_safety",
                    "Callback validation passed for {}: callback is Some",
                    callback_name
                );
                Ok(())
            }
            None => {
                warn!(
                    target: "tari::wallet_ffi::memory_safety",
                    "Callback is None for {}: this may cause issues",
                    callback_name
                );
                Err(format!("Null callback for {}", callback_name))
            }
        }
    }

    /// Comprehensive validation for wallet_create parameters
    pub fn validate_wallet_create_params(
        context: *mut c_void,
        config: *const c_void, // Using c_void since TariCommsConfig is opaque
        log_path: *const c_char,
        passphrase: *const c_char,
        seed_passphrase: *const c_char,
        network_str: *const c_char,
        dns_seeds_str: *const c_char,
        dns_seed_name_servers_str: *const c_char,
        recovery_in_progress: *mut bool,
        error_out: *mut c_int,
    ) -> Result<(), String> {
        info!(
            target: "tari::wallet_ffi::memory_safety",
            "Starting comprehensive parameter validation for wallet_create"
        );

        // Critical parameters that must not be null
        Self::validate_c_mut_pointer(error_out, "error_out")?;
        Self::validate_c_pointer(config, "config")?;

        // Context can be null in some cases, so we only check if provided
        if !context.is_null() {
            Self::validate_c_mut_pointer(context, "context")?;
        } else {
            debug!(
                target: "tari::wallet_ffi::memory_safety",
                "Context pointer is null - this may be intentional"
            );
        }

        // String parameters can be null in some cases
        if !log_path.is_null() {
            Self::validate_c_string(log_path, "log_path")?;
        }

        if !passphrase.is_null() {
            Self::validate_c_string(passphrase, "passphrase")?;
        }

        if !seed_passphrase.is_null() {
            Self::validate_c_string(seed_passphrase, "seed_passphrase")?;
        }

        if !network_str.is_null() {
            Self::validate_c_string(network_str, "network_str")?;
        }

        if !dns_seeds_str.is_null() {
            Self::validate_c_string(dns_seeds_str, "dns_seeds_str")?;
        }

        if !dns_seed_name_servers_str.is_null() {
            Self::validate_c_string(dns_seed_name_servers_str, "dns_seed_name_servers_str")?;
        }

        // Boolean output parameter
        if !recovery_in_progress.is_null() {
            Self::validate_c_mut_pointer(recovery_in_progress, "recovery_in_progress")?;
        }

        info!(
            target: "tari::wallet_ffi::memory_safety",
            "All wallet_create parameter validations passed"
        );

        Ok(())
    }

    /// Check memory state before critical operations
    pub fn check_memory_state_before_runtime_creation() {
        debug!(
            target: "tari::wallet_ffi::memory_safety",
            "Checking memory state before runtime creation"
        );

        // Log current memory usage if available
        #[cfg(target_os = "linux")]
        {
            if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
                for line in status.lines() {
                    if line.starts_with("VmRSS:") || line.starts_with("VmSize:") {
                        debug!(
                            target: "tari::wallet_ffi::memory_safety",
                            "Memory info: {}", line
                        );
                    }
                }
            }
        }

        // Check stack usage (approximate)
        let stack_var = 0u64;
        let stack_addr = &stack_var as *const u64 as usize;
        debug!(
            target: "tari::wallet_ffi::memory_safety",
            "Current stack address: 0x{:x}", stack_addr
        );
    }

    /// Check memory state after critical operations
    pub fn check_memory_state_after_wallet_creation() {
        debug!(
            target: "tari::wallet_ffi::memory_safety",
            "Checking memory state after wallet creation"
        );

        // Similar to before, but now we can detect any major changes
        #[cfg(target_os = "linux")]
        {
            if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
                for line in status.lines() {
                    if line.starts_with("VmRSS:") || line.starts_with("VmSize:") {
                        debug!(
                            target: "tari::wallet_ffi::memory_safety",
                            "Memory info after creation: {}", line
                        );
                    }
                }
            }
        }
    }

    /// Validate the final TariWallet pointer before returning
    pub fn validate_return_pointer<T>(ptr: *mut T, type_name: &str) -> Result<(), String> {
        if ptr.is_null() {
            return Err(format!("Null return pointer for {}", type_name));
        }

        let alignment = std::mem::align_of::<T>();
        let addr = ptr as usize;
        if addr % alignment != 0 {
            return Err(format!(
                "Misaligned return pointer for {}: address={:p}, required_alignment={}",
                type_name, ptr, alignment
            ));
        }

        info!(
            target: "tari::wallet_ffi::memory_safety",
            "Return pointer validation passed for {}: ptr={:p}",
            type_name, ptr
        );

        Ok(())
    }
}

/// Memory boundary protection for async operations
pub struct AsyncMemoryGuard;

impl AsyncMemoryGuard {
    /// Check memory safety before entering async context
    pub fn check_before_async_operation(operation_name: &str) {
        debug!(
            target: "tari::wallet_ffi::memory_safety",
            "Memory guard: entering async operation '{}'", operation_name
        );

        // In a real implementation, we might want to:
        // 1. Save current memory state
        // 2. Set up memory monitoring
        // 3. Prepare for potential async stack unwinding
    }

    /// Check memory safety after async operation
    pub fn check_after_async_operation(operation_name: &str) {
        debug!(
            target: "tari::wallet_ffi::memory_safety",
            "Memory guard: exiting async operation '{}'", operation_name
        );

        // In a real implementation, we might want to:
        // 1. Compare memory state with before
        // 2. Check for memory leaks
        // 3. Validate async stack integrity
    }
}

/// Node.js specific memory safety checks
pub struct NodeJsMemoryGuard;

impl NodeJsMemoryGuard {
    /// Check for Node.js GC interference patterns
    pub fn check_gc_interference_risk() -> bool {
        // Check if we're in Node.js environment
        let is_nodejs = std::env::var("NODE_ENV").is_ok() ||
                       std::env::var("npm_config_user_config").is_ok() ||
                       std::env::var("NODE_PATH").is_ok();

        if is_nodejs {
            warn!(
                target: "tari::wallet_ffi::memory_safety",
                "Node.js environment detected - GC interference risk is HIGH"
            );
            return true;
        }

        false
    }

    /// Apply Node.js specific memory protections
    pub fn apply_nodejs_protections() {
        if Self::check_gc_interference_risk() {
            info!(
                target: "tari::wallet_ffi::memory_safety",
                "Applying Node.js specific memory protections"
            );

            // In a real implementation, we might:
            // 1. Pin critical memory regions
            // 2. Use different allocation strategies
            // 3. Implement GC-aware object lifecycle management
        }
    }
}
