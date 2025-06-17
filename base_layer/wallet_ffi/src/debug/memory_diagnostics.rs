//! Memory Diagnostics and FFI Boundary Safety
//! 
//! Comprehensive memory analysis tools for identifying memory-related segfaults
//! at FFI boundaries, particularly in Node.js integration scenarios.

use std::{
    alloc::Layout,
    collections::HashMap,
    sync::{Mutex, atomic::{AtomicUsize, Ordering}},
    mem,
    ffi::CStr,
    os::raw::{c_char, c_void},
};
use log::{debug, info, warn};

/// Global memory tracking state
static MEMORY_TRACKER: Mutex<Option<MemoryTracker>> = Mutex::new(None);

/// Allocation tracking information
#[derive(Debug, Clone)]
pub struct AllocationInfo {
    pub size: usize,
    pub alignment: usize,
    pub thread_id: String,
    pub timestamp: std::time::Instant,
    pub source: AllocSource,
}

/// Source of memory allocation
#[derive(Debug, Clone, PartialEq)]
pub enum AllocSource {
    Rust,
    FFI,
    NodeJS,
    Unknown,
}

/// Memory boundary validation result
#[derive(Debug)]
pub struct BoundaryValidation {
    pub valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

/// Comprehensive memory diagnostics tracker
pub struct MemoryTracker {
    allocations: HashMap<usize, AllocationInfo>,
    total_allocated: AtomicUsize,
    total_deallocated: AtomicUsize,
    peak_usage: AtomicUsize,
    allocation_count: AtomicUsize,
    boundary_violations: AtomicUsize,
}

impl MemoryTracker {
    /// Initialize global memory tracker
    pub fn initialize() -> Result<(), Box<dyn std::error::Error>> {
        let mut tracker = MEMORY_TRACKER.lock().unwrap();
        if tracker.is_some() {
            return Ok(()); // Already initialized
        }

        *tracker = Some(Self {
            allocations: HashMap::new(),
            total_allocated: AtomicUsize::new(0),
            total_deallocated: AtomicUsize::new(0),
            peak_usage: AtomicUsize::new(0),
            allocation_count: AtomicUsize::new(0),
            boundary_violations: AtomicUsize::new(0),
        });

        info!("Memory tracker initialized");
        Ok(())
    }

    /// Record allocation
    pub fn record_allocation(ptr: *mut u8, layout: Layout, source: AllocSource) {
        if let Ok(mut tracker) = MEMORY_TRACKER.lock() {
            if let Some(ref mut tracker) = *tracker {
                let info = AllocationInfo {
                    size: layout.size(),
                    alignment: layout.align(),
                    thread_id: format!("{:?}", std::thread::current().id()),
                    timestamp: std::time::Instant::now(),
                    source,
                };

                let source = info.source.clone();
                tracker.allocations.insert(ptr as usize, info);
                let new_total = tracker.total_allocated.fetch_add(layout.size(), Ordering::Relaxed) + layout.size();
                
                // Update peak usage
                let current_peak = tracker.peak_usage.load(Ordering::Relaxed);
                if new_total > current_peak {
                    tracker.peak_usage.store(new_total, Ordering::Relaxed);
                }
                
                tracker.allocation_count.fetch_add(1, Ordering::Relaxed);

                debug!("ALLOC: {:?} bytes at {:p} from {:?}", layout.size(), ptr, source);
            }
        }
    }

    /// Record deallocation
    pub fn record_deallocation(ptr: *mut u8, layout: Layout) {
        if let Ok(mut tracker) = MEMORY_TRACKER.lock() {
            if let Some(ref mut tracker) = *tracker {
                if let Some(info) = tracker.allocations.remove(&(ptr as usize)) {
                    tracker.total_deallocated.fetch_add(layout.size(), Ordering::Relaxed);
                    debug!("DEALLOC: {} bytes at {:p} (was {:?})", layout.size(), ptr, info.source);
                } else {
                    warn!("DEALLOC: Unknown allocation at {:p}", ptr);
                    tracker.boundary_violations.fetch_add(1, Ordering::Relaxed);
                }
            }
        }
    }

    /// Get current memory statistics
    pub fn get_statistics() -> MemoryStatistics {
        if let Ok(tracker) = MEMORY_TRACKER.lock() {
            if let Some(ref tracker) = *tracker {
                return MemoryStatistics {
                    total_allocated: tracker.total_allocated.load(Ordering::Relaxed),
                    total_deallocated: tracker.total_deallocated.load(Ordering::Relaxed),
                    current_usage: tracker.total_allocated.load(Ordering::Relaxed) 
                                 - tracker.total_deallocated.load(Ordering::Relaxed),
                    peak_usage: tracker.peak_usage.load(Ordering::Relaxed),
                    allocation_count: tracker.allocation_count.load(Ordering::Relaxed),
                    boundary_violations: tracker.boundary_violations.load(Ordering::Relaxed),
                    active_allocations: tracker.allocations.len(),
                };
            }
        }

        MemoryStatistics::default()
    }

    /// Detect memory leaks
    pub fn detect_leaks() -> Vec<(usize, AllocationInfo)> {
        if let Ok(tracker) = MEMORY_TRACKER.lock() {
            if let Some(ref tracker) = *tracker {
                return tracker.allocations.iter()
                    .map(|(&addr, info)| (addr, info.clone()))
                    .collect();
            }
        }
        Vec::new()
    }
}

/// Memory usage statistics
#[derive(Debug, Default)]
pub struct MemoryStatistics {
    pub total_allocated: usize,
    pub total_deallocated: usize,
    pub current_usage: usize,
    pub peak_usage: usize,
    pub allocation_count: usize,
    pub boundary_violations: usize,
    pub active_allocations: usize,
}

/// FFI memory boundary validator
pub struct FFIBoundaryValidator;

impl FFIBoundaryValidator {
    /// Validate C string pointer from FFI
    pub fn validate_c_string(ptr: *const c_char) -> BoundaryValidation {
        let mut validation = BoundaryValidation {
            valid: true,
            errors: Vec::new(),
            warnings: Vec::new(),
        };

        if ptr.is_null() {
            validation.valid = false;
            validation.errors.push("C string pointer is null".to_string());
            return validation;
        }

        // Check alignment
        if (ptr as usize) % mem::align_of::<c_char>() != 0 {
            validation.valid = false;
            validation.errors.push("C string pointer is misaligned".to_string());
        }

        // Attempt to read the string safely
        match unsafe { CStr::from_ptr(ptr).to_str() } {
            Ok(s) => {
                if s.len() > 1024 * 1024 { // 1MB limit
                    validation.warnings.push("C string is unusually large".to_string());
                }
            }
            Err(e) => {
                validation.valid = false;
                validation.errors.push(format!("C string is invalid UTF-8: {}", e));
            }
        }

        validation
    }

    /// Validate pointer alignment for FFI
    pub fn validate_pointer_alignment<T>(ptr: *const T) -> BoundaryValidation {
        let mut validation = BoundaryValidation {
            valid: true,
            errors: Vec::new(),
            warnings: Vec::new(),
        };

        if ptr.is_null() {
            validation.valid = false;
            validation.errors.push("Pointer is null".to_string());
            return validation;
        }

        let alignment = mem::align_of::<T>();
        if (ptr as usize) % alignment != 0 {
            validation.valid = false;
            validation.errors.push(format!(
                "Pointer {:p} is misaligned (expected alignment: {})", 
                ptr, alignment
            ));
        }

        // Check if pointer is in reasonable memory range
        let addr = ptr as usize;
        if addr < 0x1000 || addr > 0x7fff_ffff_ffff_ffff {
            validation.warnings.push(format!(
                "Pointer {:p} is in suspicious memory range", ptr
            ));
        }

        validation
    }

    /// Validate memory block for FFI transfer
    pub fn validate_memory_block(ptr: *const c_void, size: usize) -> BoundaryValidation {
        let mut validation = BoundaryValidation {
            valid: true,
            errors: Vec::new(),
            warnings: Vec::new(),
        };

        if ptr.is_null() {
            validation.valid = false;
            validation.errors.push("Memory block pointer is null".to_string());
            return validation;
        }

        if size == 0 {
            validation.warnings.push("Memory block has zero size".to_string());
            return validation;
        }

        // Check for reasonable size limits (1GB)
        if size > 1024 * 1024 * 1024 {
            validation.valid = false;
            validation.errors.push("Memory block size is unreasonably large".to_string());
        }

        // Check alignment for general memory access
        if (ptr as usize) % 8 != 0 {
            validation.warnings.push("Memory block is not 8-byte aligned".to_string());
        }

        // Check if the memory range is accessible (basic sanity check)
        let start_addr = ptr as usize;
        let end_addr = start_addr.wrapping_add(size);
        
        if end_addr < start_addr {
            validation.valid = false;
            validation.errors.push("Memory block causes address overflow".to_string());
        }

        validation
    }

    /// Validate Node.js callback function pointer
    pub fn validate_callback_pointer(ptr: *const c_void) -> BoundaryValidation {
        let mut validation = BoundaryValidation {
            valid: true,
            errors: Vec::new(),
            warnings: Vec::new(),
        };

        if ptr.is_null() {
            validation.valid = false;
            validation.errors.push("Callback pointer is null".to_string());
            return validation;
        }

        // Check if pointer looks like a valid function address
        let addr = ptr as usize;
        
        // Function pointers should be aligned
        if addr % mem::align_of::<fn()>() != 0 {
            validation.valid = false;
            validation.errors.push("Callback pointer is misaligned".to_string());
        }

        // Check for reasonable address range (avoid null and very high addresses)
        if addr < 0x1000 {
            validation.valid = false;
            validation.errors.push("Callback pointer is in null pointer range".to_string());
        }

        validation
    }
}

/// Node.js garbage collection interaction monitor
pub struct GCInteractionMonitor;

impl GCInteractionMonitor {
    /// Monitor for Node.js GC interference
    pub fn detect_gc_interference() -> bool {
        // This is a heuristic check for GC interference patterns
        
        // Check for rapid allocation/deallocation patterns that suggest GC activity
        let stats = MemoryTracker::get_statistics();
        
        // If we have many boundary violations, it might indicate GC interference
        if stats.boundary_violations > 10 {
            warn!("High boundary violation count detected: {}", stats.boundary_violations);
            return true;
        }

        // Check for suspicious memory patterns
        if stats.current_usage > stats.peak_usage {
            warn!("Current usage exceeds peak usage - potential memory corruption");
            return true;
        }

        false
    }

    /// Install GC monitoring hooks (placeholder for Node.js integration)
    pub fn install_gc_hooks() {
        info!("GC interaction monitoring installed");
        // In a real implementation, this would use Node.js N-API to register GC callbacks
    }
}

/// Macro for safe FFI string handling
#[macro_export]
macro_rules! safe_ffi_string {
    ($ptr:expr) => {{
        use $crate::debug::memory_diagnostics::FFIBoundaryValidator;
        
        let validation = FFIBoundaryValidator::validate_c_string($ptr);
        if !validation.valid {
            error!("FFI string validation failed: {:?}", validation.errors);
            return std::ptr::null_mut();
        }
        
        if !validation.warnings.is_empty() {
            warn!("FFI string warnings: {:?}", validation.warnings);
        }
        
        unsafe { std::ffi::CStr::from_ptr($ptr) }
    }};
}

/// Macro for safe FFI pointer handling
#[macro_export]
macro_rules! safe_ffi_ptr {
    ($ptr:expr, $type:ty) => {{
        use $crate::debug::memory_diagnostics::FFIBoundaryValidator;
        
        let validation = FFIBoundaryValidator::validate_pointer_alignment::<$type>($ptr);
        if !validation.valid {
            error!("FFI pointer validation failed: {:?}", validation.errors);
            return std::ptr::null_mut();
        }
        
        $ptr
    }};
}

/// Initialize memory diagnostics
pub fn init_memory_diagnostics() -> Result<(), Box<dyn std::error::Error>> {
    MemoryTracker::initialize()?;
    GCInteractionMonitor::install_gc_hooks();
    info!("Memory diagnostics initialized");
    Ok(())
}

/// Generate memory diagnostic report
pub fn generate_memory_report() -> String {
    let stats = MemoryTracker::get_statistics();
    let leaks = MemoryTracker::detect_leaks();
    let gc_interference = GCInteractionMonitor::detect_gc_interference();

    let mut report = String::new();
    report.push_str("=== MEMORY DIAGNOSTIC REPORT ===\n");
    report.push_str(&format!("Total Allocated: {} bytes\n", stats.total_allocated));
    report.push_str(&format!("Total Deallocated: {} bytes\n", stats.total_deallocated));
    report.push_str(&format!("Current Usage: {} bytes\n", stats.current_usage));
    report.push_str(&format!("Peak Usage: {} bytes\n", stats.peak_usage));
    report.push_str(&format!("Allocation Count: {}\n", stats.allocation_count));
    report.push_str(&format!("Boundary Violations: {}\n", stats.boundary_violations));
    report.push_str(&format!("Active Allocations: {}\n", stats.active_allocations));
    report.push_str(&format!("GC Interference Detected: {}\n", gc_interference));

    if !leaks.is_empty() {
        report.push_str(&format!("\nMemory Leaks Detected: {}\n", leaks.len()));
        for (addr, info) in leaks.iter().take(10) { // Show first 10 leaks
            report.push_str(&format!("  {:x}: {} bytes from {:?}\n", addr, info.size, info.source));
        }
        if leaks.len() > 10 {
            report.push_str(&format!("  ... and {} more\n", leaks.len() - 10));
        }
    }

    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;

    #[test]
    fn test_memory_tracker() {
        let _ = MemoryTracker::initialize();
        
        let layout = Layout::from_size_align(64, 8).unwrap();
        let ptr = unsafe { System.alloc(layout) };
        
        MemoryTracker::record_allocation(ptr, layout, AllocSource::Rust);
        let stats = MemoryTracker::get_statistics();
        
        assert!(stats.total_allocated >= 64);
        assert!(stats.allocation_count >= 1);
        
        MemoryTracker::record_deallocation(ptr, layout);
        unsafe { System.dealloc(ptr, layout) };
    }

    #[test]
    fn test_ffi_string_validation() {
        let test_string = CString::new("Hello, World!").unwrap();
        let ptr = test_string.as_ptr();
        
        let validation = FFIBoundaryValidator::validate_c_string(ptr);
        assert!(validation.valid);
        assert!(validation.errors.is_empty());
    }

    #[test]
    fn test_pointer_alignment() {
        let test_value: u64 = 42;
        let ptr = &test_value as *const u64;
        
        let validation = FFIBoundaryValidator::validate_pointer_alignment(ptr);
        assert!(validation.valid);
    }
}
