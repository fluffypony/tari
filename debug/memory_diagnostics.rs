// Memory Diagnostics for FFI Boundary Safety
// Tools for detecting memory alignment issues, lifecycle problems, and GC interactions

use std::alloc::{GlobalAlloc, Layout, System};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::ptr;
use std::thread;
use log::{debug, error, warn, info};

/// Memory allocation tracker for debugging memory issues across FFI boundary
#[derive(Debug, Clone)]
pub struct AllocationInfo {
    pub size: usize,
    pub align: usize,
    pub thread_id: thread::ThreadId,
    pub timestamp: u64,
    pub stack_trace: String, // Simplified - in real implementation could use backtrace
}

/// Global memory tracker (for debugging builds only)
pub struct MemoryTracker {
    allocations: Arc<Mutex<HashMap<usize, AllocationInfo>>>,
    total_allocated: Arc<Mutex<usize>>,
    peak_allocated: Arc<Mutex<usize>>,
}

impl MemoryTracker {
    pub fn new() -> Self {
        Self {
            allocations: Arc::new(Mutex::new(HashMap::new())),
            total_allocated: Arc::new(Mutex::new(0)),
            peak_allocated: Arc::new(Mutex::new(0)),
        }
    }

    pub fn track_allocation(&self, ptr: *mut u8, layout: Layout) {
        if ptr.is_null() {
            return;
        }

        let addr = ptr as usize;
        let info = AllocationInfo {
            size: layout.size(),
            align: layout.align(),
            thread_id: thread::current().id(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            stack_trace: format!("Thread: {:?}", thread::current().name()),
        };

        if let Ok(mut allocations) = self.allocations.lock() {
            allocations.insert(addr, info);
        }

        if let Ok(mut total) = self.total_allocated.lock() {
            *total += layout.size();
            
            if let Ok(mut peak) = self.peak_allocated.lock() {
                if *total > *peak {
                    *peak = *total;
                }
            }
        }

        debug!(
            target: "tari::wallet_ffi::debug::memory",
            "Tracked allocation: ptr={:p}, size={}, align={}",
            ptr, layout.size(), layout.align()
        );
    }

    pub fn track_deallocation(&self, ptr: *mut u8) {
        if ptr.is_null() {
            return;
        }

        let addr = ptr as usize;
        
        if let Ok(mut allocations) = self.allocations.lock() {
            if let Some(info) = allocations.remove(&addr) {
                if let Ok(mut total) = self.total_allocated.lock() {
                    *total = total.saturating_sub(info.size);
                }
                
                debug!(
                    target: "tari::wallet_ffi::debug::memory",
                    "Tracked deallocation: ptr={:p}, size={}",
                    ptr, info.size
                );
            } else {
                warn!(
                    target: "tari::wallet_ffi::debug::memory",
                    "Attempted to deallocate untracked pointer: {:p}",
                    ptr
                );
            }
        }
    }

    pub fn get_stats(&self) -> (usize, usize, usize) {
        let current = self.total_allocated.lock().map(|t| *t).unwrap_or(0);
        let peak = self.peak_allocated.lock().map(|p| *p).unwrap_or(0);
        let active_allocations = self.allocations.lock().map(|a| a.len()).unwrap_or(0);
        
        (current, peak, active_allocations)
    }

    pub fn check_for_leaks(&self) -> Vec<(usize, AllocationInfo)> {
        let mut leaks = Vec::new();
        
        if let Ok(allocations) = self.allocations.lock() {
            for (addr, info) in allocations.iter() {
                // Consider allocations older than 5 minutes as potential leaks
                let current_time = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64;
                
                if current_time - info.timestamp > 300_000 { // 5 minutes in milliseconds
                    leaks.push(*addr, info.clone());
                }
            }
        }
        
        leaks
    }
}

/// FFI boundary memory safety checker
pub struct FfiBoundaryChecker;

impl FfiBoundaryChecker {
    /// Check if a C pointer is safe to dereference
    pub fn check_c_pointer_safety<T>(ptr: *const T) -> Result<(), String> {
        if ptr.is_null() {
            return Err("Null pointer detected".to_string());
        }

        // Check alignment
        let alignment = std::mem::align_of::<T>();
        let addr = ptr as usize;
        if addr % alignment != 0 {
            return Err(format!(
                "Misaligned pointer: address={:p}, required_alignment={}",
                ptr, alignment
            ));
        }

        // Additional platform-specific checks could go here
        // For now, we'll just log the check
        debug!(
            target: "tari::wallet_ffi::debug::memory",
            "FFI pointer safety check passed: ptr={:p}, type={}, align={}",
            ptr, std::any::type_name::<T>(), alignment
        );

        Ok(())
    }

    /// Check if a mutable C pointer is safe to write to
    pub fn check_c_mut_pointer_safety<T>(ptr: *mut T) -> Result<(), String> {
        Self::check_c_pointer_safety(ptr as *const T)?;
        
        // Additional checks for mutable pointers could go here
        debug!(
            target: "tari::wallet_ffi::debug::memory",
            "FFI mutable pointer safety check passed: ptr={:p}",
            ptr
        );

        Ok(())
    }

    /// Validate a C string pointer
    pub fn check_c_string_safety(ptr: *const std::os::raw::c_char) -> Result<(), String> {
        if ptr.is_null() {
            return Err("Null C string pointer".to_string());
        }

        // Basic alignment check
        if (ptr as usize) % std::mem::align_of::<std::os::raw::c_char>() != 0 {
            return Err("Misaligned C string pointer".to_string());
        }

        // In a real implementation, we might want to check if the string is null-terminated
        // within a reasonable bound, but that's platform-specific and potentially unsafe
        
        debug!(
            target: "tari::wallet_ffi::debug::memory",
            "C string pointer safety check passed: ptr={:p}",
            ptr
        );

        Ok(())
    }
}

/// Node.js garbage collection interaction detector
pub struct GcInteractionDetector {
    object_refs: Arc<Mutex<HashMap<usize, ObjectRefInfo>>>,
}

#[derive(Debug, Clone)]
pub struct ObjectRefInfo {
    pub created_at: u64,
    pub thread_id: thread::ThreadId,
    pub object_type: String,
    pub ref_count: usize,
}

impl GcInteractionDetector {
    pub fn new() -> Self {
        Self {
            object_refs: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Track object creation from Rust side
    pub fn track_object_creation(&self, ptr: *const std::ffi::c_void, object_type: String) {
        if ptr.is_null() {
            return;
        }

        let addr = ptr as usize;
        let info = ObjectRefInfo {
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            thread_id: thread::current().id(),
            object_type,
            ref_count: 1,
        };

        if let Ok(mut refs) = self.object_refs.lock() {
            refs.insert(addr, info);
        }

        debug!(
            target: "tari::wallet_ffi::debug::gc",
            "Tracked object creation: ptr={:p}, type={}",
            ptr, info.object_type
        );
    }

    /// Track object access (increment ref count)
    pub fn track_object_access(&self, ptr: *const std::ffi::c_void) {
        if ptr.is_null() {
            return;
        }

        let addr = ptr as usize;
        
        if let Ok(mut refs) = self.object_refs.lock() {
            if let Some(info) = refs.get_mut(&addr) {
                info.ref_count += 1;
                debug!(
                    target: "tari::wallet_ffi::debug::gc",
                    "Object access tracked: ptr={:p}, ref_count={}",
                    ptr, info.ref_count
                );
            } else {
                warn!(
                    target: "tari::wallet_ffi::debug::gc",
                    "Accessed untracked object: ptr={:p}",
                    ptr
                );
            }
        }
    }

    /// Track object destruction
    pub fn track_object_destruction(&self, ptr: *const std::ffi::c_void) {
        if ptr.is_null() {
            return;
        }

        let addr = ptr as usize;
        
        if let Ok(mut refs) = self.object_refs.lock() {
            if let Some(info) = refs.remove(&addr) {
                debug!(
                    target: "tari::wallet_ffi::debug::gc",
                    "Object destruction tracked: ptr={:p}, type={}, final_ref_count={}",
                    ptr, info.object_type, info.ref_count
                );
            } else {
                warn!(
                    target: "tari::wallet_ffi::debug::gc",
                    "Destroyed untracked object: ptr={:p}",
                    ptr
                );
            }
        }
    }

    /// Check for objects that might have been prematurely garbage collected
    pub fn detect_premature_gc(&self) -> Vec<(usize, ObjectRefInfo)> {
        let mut potential_issues = Vec::new();
        
        if let Ok(refs) = self.object_refs.lock() {
            let current_time = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;
            
            for (addr, info) in refs.iter() {
                // Flag objects that were created recently but accessed many times
                // This might indicate GC pressure
                let age_ms = current_time - info.created_at;
                if age_ms < 10_000 && info.ref_count > 100 { // Less than 10 seconds old, high access
                    potential_issues.push(*addr, info.clone());
                }
            }
        }
        
        potential_issues
    }
}

/// Comprehensive memory diagnostics runner
pub struct MemoryDiagnosticsRunner {
    tracker: MemoryTracker,
    gc_detector: GcInteractionDetector,
}

impl MemoryDiagnosticsRunner {
    pub fn new() -> Self {
        Self {
            tracker: MemoryTracker::new(),
            gc_detector: GcInteractionDetector::new(),
        }
    }

    pub fn run_full_diagnostics(&self) -> String {
        let mut report = String::new();
        report.push_str("=== Memory Diagnostics Report ===\n\n");

        // Memory allocation stats
        let (current, peak, active) = self.tracker.get_stats();
        report.push_str(&format!(
            "Memory Stats:\n  Current Allocated: {} bytes\n  Peak Allocated: {} bytes\n  Active Allocations: {}\n\n",
            current, peak, active
        ));

        // Check for memory leaks
        let leaks = self.tracker.check_for_leaks();
        if !leaks.is_empty() {
            report.push_str(&format!("Potential Memory Leaks: {}\n", leaks.len()));
            for (addr, info) in leaks.iter().take(5) { // Show first 5 leaks
                report.push_str(&format!(
                    "  Leak: addr=0x{:x}, size={}, age={}ms\n",
                    addr, info.size, 
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as u64 - info.timestamp
                ));
            }
            report.push('\n');
        }

        // Check for GC interaction issues
        let gc_issues = self.gc_detector.detect_premature_gc();
        if !gc_issues.is_empty() {
            report.push_str(&format!("Potential GC Issues: {}\n", gc_issues.len()));
            for (addr, info) in gc_issues.iter().take(5) { // Show first 5 issues
                report.push_str(&format!(
                    "  GC Issue: addr=0x{:x}, type={}, ref_count={}\n",
                    addr, info.object_type, info.ref_count
                ));
            }
            report.push('\n');
        }

        report.push_str("=== End Memory Diagnostics ===\n");
        report
    }

    pub fn get_tracker(&self) -> &MemoryTracker {
        &self.tracker
    }

    pub fn get_gc_detector(&self) -> &GcInteractionDetector {
        &self.gc_detector
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::alloc::{alloc, dealloc};

    #[test]
    fn test_memory_tracker() {
        let tracker = MemoryTracker::new();
        
        let layout = Layout::from_size_align(64, 8).unwrap();
        let ptr = unsafe { alloc(layout) };
        
        tracker.track_allocation(ptr, layout);
        
        let (current, peak, active) = tracker.get_stats();
        assert_eq!(current, 64);
        assert_eq!(peak, 64);
        assert_eq!(active, 1);
        
        tracker.track_deallocation(ptr);
        
        let (current, _, active) = tracker.get_stats();
        assert_eq!(current, 0);
        assert_eq!(active, 0);
        
        unsafe { dealloc(ptr, layout) };
    }

    #[test]
    fn test_ffi_boundary_checker() {
        let value: u64 = 42;
        let ptr = &value as *const u64;
        
        assert!(FfiBoundaryChecker::check_c_pointer_safety(ptr).is_ok());
        assert!(FfiBoundaryChecker::check_c_pointer_safety(ptr::null::<u64>()).is_err());
    }

    #[test]
    fn test_gc_interaction_detector() {
        let detector = GcInteractionDetector::new();
        
        let value: u64 = 42;
        let ptr = &value as *const u64 as *const std::ffi::c_void;
        
        detector.track_object_creation(ptr, "test_object".to_string());
        detector.track_object_access(ptr);
        detector.track_object_access(ptr);
        detector.track_object_destruction(ptr);
        
        // Should not detect any premature GC issues in this simple test
        let issues = detector.detect_premature_gc();
        assert!(issues.is_empty());
    }

    #[test]
    fn test_memory_diagnostics_runner() {
        let runner = MemoryDiagnosticsRunner::new();
        let report = runner.run_full_diagnostics();
        
        assert!(report.contains("Memory Diagnostics Report"));
        assert!(report.contains("Memory Stats"));
    }
}
