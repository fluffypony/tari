// Memory Boundary Test for FFI Safety
// Tests memory alignment, boundary crossings, and FFI safety patterns

use std::alloc::{alloc, dealloc, Layout};
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_void};
use std::ptr;
use log::{debug, error, warn, info};

/// Test FFI memory boundary safety
pub struct MemoryBoundaryTester;

impl MemoryBoundaryTester {
    /// Test C string pointer safety
    pub fn test_c_string_safety() -> Result<(), String> {
        info!(target: "tari::wallet_ffi::debug::memory_boundary", "Testing C string safety");
        
        // Test 1: Valid C string
        let test_string = CString::new("test_string").map_err(|e| e.to_string())?;
        let c_str_ptr = test_string.as_ptr();
        
        // Verify pointer alignment
        let alignment = std::mem::align_of::<c_char>();
        if (c_str_ptr as usize) % alignment != 0 {
            return Err("C string pointer misaligned".to_string());
        }
        
        // Test reading the string back
        let recovered_str = unsafe { CStr::from_ptr(c_str_ptr) };
        if recovered_str.to_string_lossy() != "test_string" {
            return Err("C string roundtrip failed".to_string());
        }
        
        debug!(target: "tari::wallet_ffi::debug::memory_boundary", "C string safety test passed");
        
        // Test 2: Null pointer handling
        let null_ptr: *const c_char = ptr::null();
        if !null_ptr.is_null() {
            return Err("Null pointer test failed".to_string());
        }
        
        // Test 3: String with special characters
        let special_string = CString::new("test_string_with_üñíçødé").map_err(|e| e.to_string())?;
        let special_ptr = special_string.as_ptr();
        let recovered_special = unsafe { CStr::from_ptr(special_ptr) };
        if recovered_special.to_string_lossy() != "test_string_with_üñíçødé" {
            return Err("Special character C string test failed".to_string());
        }
        
        info!(target: "tari::wallet_ffi::debug::memory_boundary", "All C string safety tests passed");
        Ok(())
    }
    
    /// Test pointer alignment for various data types
    pub fn test_pointer_alignment() -> Result<(), String> {
        info!(target: "tari::wallet_ffi::debug::memory_boundary", "Testing pointer alignment");
        
        // Test u64 alignment
        let value_u64: u64 = 0x123456789ABCDEF0;
        let ptr_u64 = &value_u64 as *const u64;
        let required_align_u64 = std::mem::align_of::<u64>();
        
        if (ptr_u64 as usize) % required_align_u64 != 0 {
            return Err(format!("u64 pointer misaligned: required {}, got {}", 
                              required_align_u64, (ptr_u64 as usize) % required_align_u64));
        }
        
        // Test struct alignment
        #[repr(C)]
        struct TestStruct {
            a: u32,
            b: u64,
            c: u16,
        }
        
        let test_struct = TestStruct { a: 1, b: 2, c: 3 };
        let struct_ptr = &test_struct as *const TestStruct;
        let required_align_struct = std::mem::align_of::<TestStruct>();
        
        if (struct_ptr as usize) % required_align_struct != 0 {
            return Err(format!("Struct pointer misaligned: required {}, got {}", 
                              required_align_struct, (struct_ptr as usize) % required_align_struct));
        }
        
        // Test array alignment
        let array: [u64; 10] = [0; 10];
        let array_ptr = array.as_ptr();
        let required_align_array = std::mem::align_of::<u64>();
        
        if (array_ptr as usize) % required_align_array != 0 {
            return Err(format!("Array pointer misaligned: required {}, got {}", 
                              required_align_array, (array_ptr as usize) % required_align_array));
        }
        
        info!(target: "tari::wallet_ffi::debug::memory_boundary", "All pointer alignment tests passed");
        Ok(())
    }
    
    /// Test memory allocation and deallocation safety
    pub fn test_allocation_safety() -> Result<(), String> {
        info!(target: "tari::wallet_ffi::debug::memory_boundary", "Testing allocation safety");
        
        // Test various allocation sizes and alignments
        let test_cases = vec![
            (8, 8),     // u64 alignment
            (16, 16),   // SIMD alignment
            (32, 32),   // Cache line alignment
            (64, 8),    // Larger allocation
            (128, 16),  // Even larger
            (1024, 8),  // KB-sized allocation
        ];
        
        for (size, align) in test_cases {
            debug!(target: "tari::wallet_ffi::debug::memory_boundary", 
                   "Testing allocation: size={}, align={}", size, align);
            
            let layout = Layout::from_size_align(size, align)
                .map_err(|e| format!("Invalid layout for size={}, align={}: {}", size, align, e))?;
            
            let ptr = unsafe { alloc(layout) };
            if ptr.is_null() {
                return Err(format!("Allocation failed for size={}, align={}", size, align));
            }
            
            // Check alignment
            if (ptr as usize) % align != 0 {
                unsafe { dealloc(ptr, layout) };
                return Err(format!("Allocated pointer misaligned: size={}, align={}, ptr={:p}", 
                                  size, align, ptr));
            }
            
            // Test writing to the allocated memory
            unsafe {
                ptr.write(0xAB);
                if ptr.read() != 0xAB {
                    dealloc(ptr, layout);
                    return Err(format!("Memory read/write test failed for size={}, align={}", 
                                      size, align));
                }
                dealloc(ptr, layout);
            }
        }
        
        info!(target: "tari::wallet_ffi::debug::memory_boundary", "All allocation safety tests passed");
        Ok(())
    }
    
    /// Test FFI function parameter boundary safety
    pub fn test_ffi_parameter_safety() -> Result<(), String> {
        info!(target: "tari::wallet_ffi::debug::memory_boundary", "Testing FFI parameter safety");
        
        // Simulate the pattern used in wallet_create
        
        // Test 1: Context pointer (void*)
        let context_data: u64 = 0x12345678;
        let context_ptr = &context_data as *const u64 as *mut c_void;
        
        if context_ptr.is_null() {
            return Err("Context pointer is null".to_string());
        }
        
        // Test recovery
        let recovered_context = unsafe { *(context_ptr as *const u64) };
        if recovered_context != context_data {
            return Err("Context pointer roundtrip failed".to_string());
        }
        
        // Test 2: Error output parameter
        let mut error_out: c_int = 0;
        let error_ptr = &mut error_out as *mut c_int;
        
        if error_ptr.is_null() {
            return Err("Error output pointer is null".to_string());
        }
        
        // Simulate setting error
        unsafe { *error_ptr = 42 };
        if error_out != 42 {
            return Err("Error output parameter test failed".to_string());
        }
        
        // Test 3: Boolean output parameter
        let mut bool_out: bool = false;
        let bool_ptr = &mut bool_out as *mut bool;
        
        if bool_ptr.is_null() {
            return Err("Boolean output pointer is null".to_string());
        }
        
        unsafe { *bool_ptr = true };
        if !bool_out {
            return Err("Boolean output parameter test failed".to_string());
        }
        
        info!(target: "tari::wallet_ffi::debug::memory_boundary", "All FFI parameter safety tests passed");
        Ok(())
    }
    
    /// Test callback function pointer safety
    pub fn test_callback_safety() -> Result<(), String> {
        info!(target: "tari::wallet_ffi::debug::memory_boundary", "Testing callback safety");
        
        // Define a test callback function
        unsafe extern "C" fn test_callback(
            context: *mut c_void,
            value: c_int,
        ) {
            if !context.is_null() {
                let data_ptr = context as *mut u32;
                *data_ptr = value as u32;
            }
        }
        
        // Test callback invocation
        let mut callback_data: u32 = 0;
        let context = &mut callback_data as *mut u32 as *mut c_void;
        
        unsafe {
            test_callback(context, 123);
        }
        
        if callback_data != 123 {
            return Err("Callback invocation test failed".to_string());
        }
        
        // Test null callback handling
        let null_callback: Option<unsafe extern "C" fn(*mut c_void, c_int)> = None;
        
        match null_callback {
            Some(_) => return Err("Null callback detection failed".to_string()),
            None => {
                debug!(target: "tari::wallet_ffi::debug::memory_boundary", 
                       "Null callback correctly detected");
            }
        }
        
        info!(target: "tari::wallet_ffi::debug::memory_boundary", "All callback safety tests passed");
        Ok(())
    }
    
    /// Run comprehensive memory boundary tests
    pub fn run_all_tests() -> Result<(), String> {
        info!(target: "tari::wallet_ffi::debug::memory_boundary", 
              "Starting comprehensive memory boundary tests");
        
        Self::test_c_string_safety()?;
        Self::test_pointer_alignment()?;
        Self::test_allocation_safety()?;
        Self::test_ffi_parameter_safety()?;
        Self::test_callback_safety()?;
        
        info!(target: "tari::wallet_ffi::debug::memory_boundary", 
              "All memory boundary tests passed successfully");
        Ok(())
    }
    
    /// Generate memory boundary test report
    pub fn generate_test_report() -> String {
        let mut report = String::new();
        report.push_str("=== Memory Boundary Test Report ===\n\n");
        
        match Self::run_all_tests() {
            Ok(()) => {
                report.push_str("RESULT: ALL TESTS PASSED\n\n");
                report.push_str("Memory boundary safety verification successful:\n");
                report.push_str("  ✓ C string safety\n");
                report.push_str("  ✓ Pointer alignment\n");
                report.push_str("  ✓ Allocation safety\n");
                report.push_str("  ✓ FFI parameter safety\n");
                report.push_str("  ✓ Callback safety\n");
            }
            Err(e) => {
                report.push_str("RESULT: TESTS FAILED\n\n");
                report.push_str(&format!("Error: {}\n", e));
                report.push_str("\nThis indicates potential memory safety issues that could cause segfaults.\n");
            }
        }
        
        report.push_str("\n=== End Memory Boundary Test Report ===\n");
        report
    }
}

/// Test Node.js specific memory patterns
pub struct NodeJsMemoryTester;

impl NodeJsMemoryTester {
    /// Test memory patterns that might conflict with Node.js GC
    pub fn test_gc_interaction_patterns() -> Result<(), String> {
        info!(target: "tari::wallet_ffi::debug::memory_boundary", 
              "Testing Node.js GC interaction patterns");
        
        // Test 1: Rapidly allocate and deallocate memory (simulating GC pressure)
        for i in 0..100 {
            let layout = Layout::from_size_align(1024, 8)
                .map_err(|e| format!("Layout creation failed: {}", e))?;
            
            let ptr = unsafe { alloc(layout) };
            if ptr.is_null() {
                return Err(format!("Allocation failed on iteration {}", i));
            }
            
            // Write some data
            unsafe {
                for j in 0..1024 {
                    ptr.add(j).write(((i + j) % 256) as u8);
                }
                
                // Verify data
                for j in 0..1024 {
                    let expected = ((i + j) % 256) as u8;
                    let actual = ptr.add(j).read();
                    if actual != expected {
                        dealloc(ptr, layout);
                        return Err(format!("Data corruption detected on iteration {}, offset {}", i, j));
                    }
                }
                
                dealloc(ptr, layout);
            }
        }
        
        info!(target: "tari::wallet_ffi::debug::memory_boundary", 
              "GC interaction pattern test passed");
        Ok(())
    }
    
    /// Test long-lived object patterns
    pub fn test_long_lived_objects() -> Result<(), String> {
        info!(target: "tari::wallet_ffi::debug::memory_boundary", 
              "Testing long-lived object patterns");
        
        // Simulate the TariWallet object lifecycle
        let layout = Layout::from_size_align(std::mem::size_of::<u64>() * 10, 8)
            .map_err(|e| format!("Layout creation failed: {}", e))?;
        
        let wallet_ptr = unsafe { alloc(layout) };
        if wallet_ptr.is_null() {
            return Err("Wallet object allocation failed".to_string());
        }
        
        // Initialize the "wallet" with some data
        unsafe {
            let wallet_data = wallet_ptr as *mut u64;
            for i in 0..10 {
                wallet_data.add(i).write(0xDEADBEEF00000000 + i as u64);
            }
            
            // Simulate long-term usage
            for iteration in 0..1000 {
                // Read some data
                let value = wallet_data.add(iteration % 10).read();
                let expected = 0xDEADBEEF00000000 + (iteration % 10) as u64;
                
                if value != expected {
                    dealloc(ptr, layout);
                    return Err(format!("Long-lived object data corruption on iteration {}", iteration));
                }
                
                // Modify some data
                wallet_data.add(iteration % 10).write(expected + 1);
                wallet_data.add(iteration % 10).write(expected); // Restore
            }
            
            dealloc(wallet_ptr, layout);
        }
        
        info!(target: "tari::wallet_ffi::debug::memory_boundary", 
              "Long-lived object pattern test passed");
        Ok(())
    }
    
    /// Run Node.js specific memory tests
    pub fn run_nodejs_tests() -> Result<(), String> {
        info!(target: "tari::wallet_ffi::debug::memory_boundary", 
              "Starting Node.js specific memory tests");
        
        Self::test_gc_interaction_patterns()?;
        Self::test_long_lived_objects()?;
        
        info!(target: "tari::wallet_ffi::debug::memory_boundary", 
              "All Node.js memory tests passed");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_boundary_tests() {
        let result = MemoryBoundaryTester::run_all_tests();
        assert!(result.is_ok(), "Memory boundary tests should pass: {:?}", result);
    }

    #[test]
    fn test_nodejs_memory_patterns() {
        let result = NodeJsMemoryTester::run_nodejs_tests();
        assert!(result.is_ok(), "Node.js memory tests should pass: {:?}", result);
    }

    #[test]
    fn test_report_generation() {
        let report = MemoryBoundaryTester::generate_test_report();
        assert!(report.contains("Memory Boundary Test Report"));
        assert!(report.contains("RESULT:"));
    }
}
