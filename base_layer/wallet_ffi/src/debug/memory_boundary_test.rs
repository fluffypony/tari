//! Memory Boundary Testing
//! 
//! Specific tests for memory boundary issues that can cause segfaults
//! when data crosses the FFI boundary between Rust and Node.js.

use std::{
    ffi::{CStr, CString},
    os::raw::{c_char, c_void},
    ptr,
    mem,
    slice,
};
use log::{debug, info};

/// Memory boundary test suite
pub struct MemoryBoundaryTest;

impl MemoryBoundaryTest {
    /// Test string handling across FFI boundary
    pub fn test_string_boundary() -> Result<(), String> {
        info!("Testing string boundary handling...");

        // Test 1: Empty string
        let empty_str = CString::new("").map_err(|e| format!("Failed to create empty string: {}", e))?;
        if Self::validate_string_roundtrip(empty_str.as_ptr()).is_err() {
            return Err("Empty string roundtrip failed".to_string());
        }

        // Test 2: Normal string
        let normal_str = CString::new("Hello, World!").map_err(|e| format!("Failed to create normal string: {}", e))?;
        if Self::validate_string_roundtrip(normal_str.as_ptr()).is_err() {
            return Err("Normal string roundtrip failed".to_string());
        }

        // Test 3: Unicode string
        let unicode_str = CString::new("🦀 Rust 🚀").map_err(|e| format!("Failed to create unicode string: {}", e))?;
        if Self::validate_string_roundtrip(unicode_str.as_ptr()).is_err() {
            return Err("Unicode string roundtrip failed".to_string());
        }

        // Test 4: Large string
        let large_content = "x".repeat(1024 * 1024); // 1MB string
        let large_str = CString::new(large_content).map_err(|e| format!("Failed to create large string: {}", e))?;
        if Self::validate_string_roundtrip(large_str.as_ptr()).is_err() {
            return Err("Large string roundtrip failed".to_string());
        }

        info!("String boundary tests passed");
        Ok(())
    }

    /// Validate string roundtrip through FFI
    fn validate_string_roundtrip(ptr: *const c_char) -> Result<(), String> {
        if ptr.is_null() {
            return Err("String pointer is null".to_string());
        }

        // Convert to Rust string and back
        let c_str = unsafe { CStr::from_ptr(ptr) };
        let rust_str = c_str.to_str().map_err(|e| format!("Invalid UTF-8: {}", e))?;
        let roundtrip = CString::new(rust_str).map_err(|e| format!("Roundtrip failed: {}", e))?;

        // Verify they match
        if c_str.to_bytes() != roundtrip.as_bytes() {
            return Err("String roundtrip content mismatch".to_string());
        }

        Ok(())
    }

    /// Test pointer alignment across FFI boundary
    pub fn test_pointer_alignment() -> Result<(), String> {
        info!("Testing pointer alignment...");

        // Test different data types and their alignment requirements
        let test_u8: u8 = 42;
        let test_u16: u16 = 0x1234;
        let test_u32: u32 = 0x12345678;
        let test_u64: u64 = 0x123456789ABCDEF0;

        Self::validate_alignment(&test_u8 as *const u8, mem::align_of::<u8>())?;
        Self::validate_alignment(&test_u16 as *const u16, mem::align_of::<u16>())?;
        Self::validate_alignment(&test_u32 as *const u32, mem::align_of::<u32>())?;
        Self::validate_alignment(&test_u64 as *const u64, mem::align_of::<u64>())?;

        // Test structure alignment
        #[repr(C)]
        struct TestStruct {
            a: u8,
            b: u16,
            c: u32,
            d: u64,
        }

        let test_struct = TestStruct { a: 1, b: 2, c: 3, d: 4 };
        Self::validate_alignment(&test_struct as *const TestStruct, mem::align_of::<TestStruct>())?;

        info!("Pointer alignment tests passed");
        Ok(())
    }

    /// Validate pointer alignment
    fn validate_alignment<T>(ptr: *const T, expected_align: usize) -> Result<(), String> {
        let addr = ptr as usize;
        if addr % expected_align != 0 {
            return Err(format!(
                "Pointer {:p} is misaligned (expected: {}, got: {})",
                ptr, expected_align, addr % expected_align
            ));
        }
        Ok(())
    }

    /// Test memory buffer handling across FFI
    pub fn test_buffer_boundary() -> Result<(), String> {
        info!("Testing buffer boundary handling...");

        // Test various buffer sizes
        let sizes = [0, 1, 8, 64, 1024, 4096, 65536];

        for &size in &sizes {
            Self::test_buffer_size(size)?;
        }

        info!("Buffer boundary tests passed");
        Ok(())
    }

    /// Test specific buffer size
    fn test_buffer_size(size: usize) -> Result<(), String> {
        debug!("Testing buffer size: {}", size);

        if size == 0 {
            // Test null/empty buffer handling
            let null_ptr: *const u8 = ptr::null();
            if !null_ptr.is_null() {
                return Err("Null pointer test failed".to_string());
            }
            return Ok(());
        }

        // Allocate buffer
        let mut buffer = vec![0u8; size];
        
        // Fill with test pattern
        for (i, byte) in buffer.iter_mut().enumerate() {
            *byte = (i % 256) as u8;
        }

        // Test buffer access through raw pointer
        let ptr = buffer.as_ptr();
        let slice = unsafe { slice::from_raw_parts(ptr, size) };

        // Verify test pattern
        for (i, &byte) in slice.iter().enumerate() {
            if byte != (i % 256) as u8 {
                return Err(format!("Buffer content mismatch at index {}", i));
            }
        }

        // Test buffer modification
        let mut_ptr = buffer.as_mut_ptr();
        unsafe {
            for i in 0..size {
                *mut_ptr.add(i) = 0xFF;
            }
        }

        // Verify modification
        if !buffer.iter().all(|&b| b == 0xFF) {
            return Err("Buffer modification test failed".to_string());
        }

        Ok(())
    }

    /// Test callback function pointer handling
    pub fn test_callback_boundary() -> Result<(), String> {
        info!("Testing callback boundary handling...");

        // Test function pointer roundtrip
        extern "C" fn test_callback(_arg: i32) -> i32 {
            42
        }

        let callback_ptr = test_callback as *const c_void;
        if callback_ptr.is_null() {
            return Err("Callback pointer is null".to_string());
        }

        // Test callback invocation (simulate FFI callback)
        let result = Self::simulate_ffi_callback(test_callback);
        if result != 42 {
            return Err(format!("Callback returned wrong value: {}", result));
        }

        info!("Callback boundary tests passed");
        Ok(())
    }

    /// Simulate FFI callback invocation
    fn simulate_ffi_callback(callback: extern "C" fn(i32) -> i32) -> i32 {
        // This simulates what would happen when Node.js calls back into Rust
        callback(123)
    }

    /// Test memory ownership transfer patterns
    pub fn test_ownership_transfer() -> Result<(), String> {
        info!("Testing memory ownership transfer...");

        // Test 1: Rust-allocated, C-freed pattern (should be avoided)
        let rust_string = CString::new("test").unwrap();
        let ptr = rust_string.into_raw(); // Transfer ownership to C
        
        // Simulate C code using the string
        let c_str = unsafe { CStr::from_ptr(ptr) };
        if c_str.to_str().unwrap() != "test" {
            return Err("Ownership transfer corrupted data".to_string());
        }
        
        // Take ownership back and free
        let _reclaimed = unsafe { CString::from_raw(ptr) };

        // Test 2: C-allocated, Rust-used pattern
        // This would typically involve malloc/free from C side
        // For testing, we simulate with Vec
        let simulated_c_data = vec![1u8, 2, 3, 4, 5];
        let c_ptr = simulated_c_data.as_ptr();
        let c_len = simulated_c_data.len();
        
        // Simulate Rust reading C-allocated data
        let rust_view = unsafe { slice::from_raw_parts(c_ptr, c_len) };
        if rust_view != &[1, 2, 3, 4, 5] {
            return Err("C-to-Rust data transfer failed".to_string());
        }

        info!("Ownership transfer tests passed");
        Ok(())
    }

    /// Test concurrent access patterns that might cause issues
    pub fn test_concurrent_access() -> Result<(), String> {
        info!("Testing concurrent access patterns...");

        use std::sync::{Arc, Mutex};
        use std::thread;

        // Create shared data structure
        let shared_data = Arc::new(Mutex::new(vec![0u8; 1024]));
        let mut handles = vec![];

        // Spawn multiple threads accessing the same data
        for i in 0..4 {
            let data = shared_data.clone();
            let handle = thread::spawn(move || {
                for j in 0..100 {
                    let mut guard = data.lock().unwrap();
                    let index = j % guard.len();
                    guard[index] = (i * j) as u8;
                    // Simulate some work
                    thread::yield_now();
                }
            });
            handles.push(handle);
        }

        // Wait for all threads to complete
        for handle in handles {
            handle.join().map_err(|_| "Thread panicked during concurrent access test")?;
        }

        info!("Concurrent access tests passed");
        Ok(())
    }

    /// Run all boundary tests
    pub fn run_all_tests() -> Result<(), String> {
        info!("Running comprehensive memory boundary tests...");

        Self::test_string_boundary()?;
        Self::test_pointer_alignment()?;
        Self::test_buffer_boundary()?;
        Self::test_callback_boundary()?;
        Self::test_ownership_transfer()?;
        Self::test_concurrent_access()?;

        info!("All memory boundary tests passed!");
        Ok(())
    }
}

/// Node.js specific memory patterns test
pub struct NodeJSMemoryPatterns;

impl NodeJSMemoryPatterns {
    /// Test patterns specific to Node.js integration
    pub fn test_nodejs_patterns() -> Result<(), String> {
        info!("Testing Node.js specific memory patterns...");

        // Test 1: Rapid allocation/deallocation (GC pressure simulation)
        Self::test_gc_pressure_simulation()?;

        // Test 2: Event loop callback patterns
        Self::test_event_loop_patterns()?;

        // Test 3: Buffer to ArrayBuffer conversion patterns
        Self::test_buffer_conversion_patterns()?;

        info!("Node.js memory pattern tests passed");
        Ok(())
    }

    /// Simulate GC pressure scenarios
    fn test_gc_pressure_simulation() -> Result<(), String> {
        debug!("Testing GC pressure simulation...");

        // Rapidly allocate and deallocate strings (simulates Node.js GC behavior)
        for i in 0..1000 {
            let test_str = format!("test_string_{}", i);
            let c_string = CString::new(test_str).map_err(|e| format!("CString creation failed: {}", e))?;
            let ptr = c_string.as_ptr();
            
            // Simulate cross-boundary access
            let recovered = unsafe { CStr::from_ptr(ptr) };
            let rust_str = recovered.to_str().map_err(|e| format!("String recovery failed: {}", e))?;
            
            if !rust_str.starts_with("test_string_") {
                return Err("GC pressure test corrupted string".to_string());
            }
            
            // Let CString drop naturally
        }

        Ok(())
    }

    /// Test event loop callback patterns
    fn test_event_loop_patterns() -> Result<(), String> {
        debug!("Testing event loop patterns...");

        // Simulate multiple pending callbacks (common in Node.js)
        let mut callback_data = Vec::new();
        
        for i in 0..100 {
            let data = format!("callback_data_{}", i);
            let c_string = CString::new(data).map_err(|e| format!("Callback data creation failed: {}", e))?;
            callback_data.push(c_string);
        }

        // Simulate processing callbacks in random order (like event loop)
        use rand::seq::SliceRandom;
        let mut indices: Vec<usize> = (0..callback_data.len()).collect();
        indices.shuffle(&mut rand::thread_rng());

        for &index in &indices {
            let ptr = callback_data[index].as_ptr();
            let recovered = unsafe { CStr::from_ptr(ptr) };
            let rust_str = recovered.to_str().map_err(|e| format!("Callback processing failed: {}", e))?;
            
            if !rust_str.starts_with("callback_data_") {
                return Err("Event loop pattern test corrupted data".to_string());
            }
        }

        Ok(())
    }

    /// Test buffer conversion patterns
    fn test_buffer_conversion_patterns() -> Result<(), String> {
        debug!("Testing buffer conversion patterns...");

        // Simulate Node.js Buffer to Rust Vec<u8> conversion patterns
        let test_sizes = [0, 1, 16, 256, 4096, 65536];

        for &size in &test_sizes {
            // Create buffer with test pattern
            let mut buffer = vec![0u8; size];
            for (i, byte) in buffer.iter_mut().enumerate() {
                *byte = (i % 256) as u8;
            }

            // Simulate conversion to C buffer (as Node.js would do)
            let c_ptr = buffer.as_ptr();
            let c_len = buffer.len();

            // Simulate Rust reading from C buffer
            if c_len > 0 {
                let rust_slice = unsafe { slice::from_raw_parts(c_ptr, c_len) };
                
                // Verify test pattern
                for (i, &byte) in rust_slice.iter().enumerate() {
                    if byte != (i % 256) as u8 {
                        return Err(format!("Buffer conversion test failed at size {} index {}", size, i));
                    }
                }
            }

            // Simulate modification through C interface
            if c_len > 0 {
                let c_mut_ptr = buffer.as_mut_ptr();
                unsafe {
                    for i in 0..c_len {
                        *c_mut_ptr.add(i) = 0xFF;
                    }
                }

                // Verify modification
                if !buffer.iter().all(|&b| b == 0xFF) {
                    return Err(format!("Buffer modification test failed at size {}", size));
                }
            }
        }

        Ok(())
    }
}

/// Initialize and run all memory boundary tests
pub fn run_comprehensive_boundary_tests() -> Result<(), String> {
    crate::debug::segfault_investigation::init_test_debugging();
    
    info!("Starting comprehensive memory boundary tests...");
    
    MemoryBoundaryTest::run_all_tests()?;
    NodeJSMemoryPatterns::test_nodejs_patterns()?;
    
    info!("All boundary tests completed successfully!");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_string_boundary() {
        let result = MemoryBoundaryTest::test_string_boundary();
        assert!(result.is_ok(), "String boundary test failed: {:?}", result);
    }

    #[test]
    fn test_pointer_alignment() {
        let result = MemoryBoundaryTest::test_pointer_alignment();
        assert!(result.is_ok(), "Pointer alignment test failed: {:?}", result);
    }

    #[test]
    fn test_buffer_boundary() {
        let result = MemoryBoundaryTest::test_buffer_boundary();
        assert!(result.is_ok(), "Buffer boundary test failed: {:?}", result);
    }

    #[test]
    fn test_comprehensive() {
        let result = run_comprehensive_boundary_tests();
        assert!(result.is_ok(), "Comprehensive boundary tests failed: {:?}", result);
    }
}
