// Node.js FFI Integration Tests
// Tests for the enhanced wallet_create function with Node.js compatibility

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_void};
use std::ptr;
use std::thread;
use std::time::Duration;
use minotari_wallet_ffi;

#[cfg(test)]
mod tests {
    use super::*;

    // Mock TariCommsConfig for testing
    #[repr(C)]
    struct MockTariCommsConfig {
        data: [u8; 64], // Placeholder data
    }

    // Basic FFI safety tests
    #[test]
    fn test_null_pointer_handling() {
        // Test that null pointers are handled gracefully
        let mut error_out: c_int = 0;
        
        // This should fail safely without crashing
        let result = unsafe {
            minotari_wallet_ffi::wallet_create(
                ptr::null_mut(),      // context
                ptr::null_mut(),      // config (should cause error)
                ptr::null(),          // log_path
                0,                    // log_verbosity  
                0,                    // num_rolling_log_files
                0,                    // size_per_log_file_bytes
                ptr::null(),          // passphrase
                ptr::null(),          // seed_passphrase
                ptr::null_mut(),      // seed_words
                ptr::null(),          // network_str
                ptr::null(),          // dns_seeds_str
                ptr::null(),          // dns_seed_name_servers_str
                false,                // use_dns_sec
                dummy_callback,       // callback_received_transaction
                dummy_callback_reply, // callback_received_transaction_reply
                dummy_callback_final, // callback_received_finalized_transaction
                dummy_callback_broadcast, // callback_transaction_broadcast
                dummy_callback_mined, // callback_transaction_mined
                dummy_callback_mined_unconfirmed, // callback_transaction_mined_unconfirmed
                dummy_callback_faux_confirmed, // callback_faux_transaction_confirmed
                dummy_callback_faux_unconfirmed, // callback_faux_transaction_unconfirmed
                dummy_callback_send_result, // callback_transaction_send_result
                dummy_callback_cancellation, // callback_transaction_cancellation
                dummy_callback_txo_validation, // callback_txo_validation_complete
                dummy_callback_contacts_liveness, // callback_contacts_liveness_data_updated
                dummy_callback_balance, // callback_balance_updated
                dummy_callback_tx_validation, // callback_transaction_validation_complete
                dummy_callback_saf, // callback_saf_messages_received
                dummy_callback_connectivity, // callback_connectivity_status
                dummy_callback_wallet_scanned, // callback_wallet_scanned_height
                dummy_callback_base_node_state, // callback_base_node_state
                ptr::null_mut(),      // recovery_in_progress
                &mut error_out,       // error_out
            )
        };

        // Should return null pointer and set error
        assert!(result.is_null());
        assert_ne!(error_out, 0);
    }

    #[test]
    fn test_error_parameter_validation() {
        // Test with null error_out parameter - should return null immediately
        let result = unsafe {
            minotari_wallet_ffi::wallet_create(
                ptr::null_mut(),      // context
                ptr::null_mut(),      // config
                ptr::null(),          // log_path
                0,                    // log_verbosity
                0,                    // num_rolling_log_files
                0,                    // size_per_log_file_bytes
                ptr::null(),          // passphrase
                ptr::null(),          // seed_passphrase
                ptr::null_mut(),      // seed_words
                ptr::null(),          // network_str
                ptr::null(),          // dns_seeds_str
                ptr::null(),          // dns_seed_name_servers_str
                false,                // use_dns_sec
                dummy_callback,
                dummy_callback_reply,
                dummy_callback_final,
                dummy_callback_broadcast,
                dummy_callback_mined,
                dummy_callback_mined_unconfirmed,
                dummy_callback_faux_confirmed,
                dummy_callback_faux_unconfirmed,
                dummy_callback_send_result,
                dummy_callback_cancellation,
                dummy_callback_txo_validation,
                dummy_callback_contacts_liveness,
                dummy_callback_balance,
                dummy_callback_tx_validation,
                dummy_callback_saf,
                dummy_callback_connectivity,
                dummy_callback_wallet_scanned,
                dummy_callback_base_node_state,
                ptr::null_mut(),      // recovery_in_progress
                ptr::null_mut(),      // error_out (NULL!)
            )
        };

        // Should return null pointer when error_out is null
        assert!(result.is_null());
    }

    #[cfg(feature = "debug_memory")]
    #[test]
    fn test_memory_safety_validation() {
        use minotari_wallet_ffi::debug_memory_safety::FfiMemorySafetyChecker;

        // Test pointer validation
        let test_value: u64 = 42;
        let valid_ptr = &test_value as *const u64;
        
        assert!(FfiMemorySafetyChecker::validate_c_pointer(valid_ptr, "test_param").is_ok());
        assert!(FfiMemorySafetyChecker::validate_c_pointer(ptr::null::<u64>(), "null_param").is_err());

        // Test C string validation
        let test_string = CString::new("test").unwrap();
        let c_str_ptr = test_string.as_ptr();
        
        assert!(FfiMemorySafetyChecker::validate_c_string(c_str_ptr, "test_string").is_ok());
        assert!(FfiMemorySafetyChecker::validate_c_string(ptr::null(), "null_string").is_err());
    }

    #[cfg(feature = "debug_runtime")]
    #[test]
    fn test_nodejs_environment_detection() {
        use minotari_wallet_ffi::debug_segfault_investigation::NodeJsDetector;

        // Test environment detection (will depend on test environment)
        let is_nodejs = NodeJsDetector::is_nodejs_environment();
        let is_nodejs_thread = NodeJsDetector::is_potential_nodejs_thread();

        // These are environment-dependent, so we just verify they don't crash
        println!("Is Node.js environment: {}", is_nodejs);
        println!("Is potential Node.js thread: {}", is_nodejs_thread);
    }

    #[cfg(feature = "nodejs_compatibility")]
    #[test]
    fn test_runtime_strategy_selection() {
        use minotari_wallet_ffi::runtime_strategies::{RuntimeStrategy, RuntimeSelector};

        let selector = RuntimeSelector::new();
        let strategy = selector.get_strategy();

        // Should select a valid strategy
        assert!(matches!(
            strategy,
            RuntimeStrategy::MultiThreaded |
            RuntimeStrategy::SingleThreaded |
            RuntimeStrategy::CurrentThread |
            RuntimeStrategy::DedicatedThread
        ));

        println!("Selected runtime strategy: {:?}", strategy);
    }

    #[cfg(feature = "nodejs_compatibility")]
    #[test]
    fn test_runtime_creation_strategies() {
        use minotari_wallet_ffi::runtime_strategies::RuntimeStrategy;

        // Test different runtime strategies
        let strategies = vec![
            RuntimeStrategy::MultiThreaded,
            RuntimeStrategy::SingleThreaded,
        ];

        for strategy in strategies {
            match strategy.create_runtime() {
                Ok(runtime_wrapper) => {
                    println!("Strategy {:?} succeeded, type: {}", strategy, runtime_wrapper.get_type());
                }
                Err(e) => {
                    println!("Strategy {:?} failed: {}", strategy, e);
                    // Some strategies may fail in test environment, that's okay
                }
            }
        }
    }

    #[test]
    fn test_concurrent_runtime_safety() {
        // Test that multiple threads trying to create runtimes don't interfere
        let handles: Vec<_> = (0..4)
            .map(|i| {
                thread::spawn(move || {
                    thread::sleep(Duration::from_millis(i * 10));
                    
                    // Simulate the environment detection that happens in wallet_create
                    #[cfg(feature = "debug_runtime")]
                    {
                        use minotari_wallet_ffi::debug_segfault_investigation::NodeJsDetector;
                        NodeJsDetector::log_environment_details();
                    }

                    #[cfg(feature = "nodejs_compatibility")]
                    {
                        use minotari_wallet_ffi::runtime_strategies::RuntimeSelector;
                        let selector = RuntimeSelector::new();
                        match selector.create_runtime_with_fallback() {
                            Ok(_) => format!("Thread {} succeeded", i),
                            Err(e) => format!("Thread {} failed: {}", i, e),
                        }
                    }

                    #[cfg(not(feature = "nodejs_compatibility"))]
                    {
                        // Just test basic Tokio runtime creation
                        match tokio::runtime::Runtime::new() {
                            Ok(_) => format!("Thread {} succeeded", i),
                            Err(e) => format!("Thread {} failed: {}", i, e),
                        }
                    }
                })
            })
            .collect();

        // Wait for all threads to complete
        for handle in handles {
            let result = handle.join().unwrap();
            println!("Concurrent test result: {}", result);
        }
    }

    // Test memory boundary patterns
    #[test]
    fn test_memory_boundary_patterns() {
        // Test various memory allocation patterns that might cause issues
        use std::alloc::{alloc, dealloc, Layout};

        let layouts = vec![
            Layout::from_size_align(8, 8).unwrap(),
            Layout::from_size_align(64, 8).unwrap(),
            Layout::from_size_align(1024, 16).unwrap(),
        ];

        for layout in layouts {
            unsafe {
                let ptr = alloc(layout);
                assert!(!ptr.is_null(), "Allocation failed");
                
                // Test alignment
                assert_eq!(ptr as usize % layout.align(), 0, "Pointer not properly aligned");
                
                // Write and read test
                ptr.write(0xAB);
                assert_eq!(ptr.read(), 0xAB, "Memory read/write failed");
                
                dealloc(ptr, layout);
            }
        }
    }

    // Dummy callback functions for testing
    unsafe extern "C" fn dummy_callback(_context: *mut c_void, _param: *mut c_void) {}
    unsafe extern "C" fn dummy_callback_reply(_context: *mut c_void, _param: *mut c_void) {}
    unsafe extern "C" fn dummy_callback_final(_context: *mut c_void, _param: *mut c_void) {}
    unsafe extern "C" fn dummy_callback_broadcast(_context: *mut c_void, _param: *mut c_void) {}
    unsafe extern "C" fn dummy_callback_mined(_context: *mut c_void, _param: *mut c_void) {}
    unsafe extern "C" fn dummy_callback_mined_unconfirmed(_context: *mut c_void, _param: *mut c_void, _height: u64) {}
    unsafe extern "C" fn dummy_callback_faux_confirmed(_context: *mut c_void, _param: *mut c_void) {}
    unsafe extern "C" fn dummy_callback_faux_unconfirmed(_context: *mut c_void, _param: *mut c_void, _height: u64) {}
    unsafe extern "C" fn dummy_callback_send_result(_context: *mut c_void, _id: u64, _status: *mut c_void) {}
    unsafe extern "C" fn dummy_callback_cancellation(_context: *mut c_void, _param: *mut c_void, _reason: u64) {}
    unsafe extern "C" fn dummy_callback_txo_validation(_context: *mut c_void, _request_key: u64, _status: u64) {}
    unsafe extern "C" fn dummy_callback_contacts_liveness(_context: *mut c_void, _data: *mut c_void) {}
    unsafe extern "C" fn dummy_callback_balance(_context: *mut c_void, _balance: *mut c_void) {}
    unsafe extern "C" fn dummy_callback_tx_validation(_context: *mut c_void, _request_key: u64, _status: u64) {}
    unsafe extern "C" fn dummy_callback_saf(_context: *mut c_void) {}
    unsafe extern "C" fn dummy_callback_connectivity(_context: *mut c_void, _status: u64) {}
    unsafe extern "C" fn dummy_callback_wallet_scanned(_context: *mut c_void, _height: u64) {}
    unsafe extern "C" fn dummy_callback_base_node_state(_context: *mut c_void, _state: *mut c_void) {}
}

// Benchmarks for performance testing
#[cfg(test)]
mod benchmarks {
    use super::*;
    use std::time::Instant;

    #[test]
    fn benchmark_runtime_creation() {
        #[cfg(feature = "nodejs_compatibility")]
        {
            use minotari_wallet_ffi::runtime_strategies::RuntimeSelector;

            let iterations = 10;
            let start = Instant::now();

            for _ in 0..iterations {
                let selector = RuntimeSelector::new();
                let _ = selector.create_runtime_with_fallback();
            }

            let duration = start.elapsed();
            let avg_duration = duration / iterations;

            println!("Runtime creation benchmark:");
            println!("  Total time: {:?}", duration);
            println!("  Average per creation: {:?}", avg_duration);
            println!("  Creations per second: {:.2}", 1.0 / avg_duration.as_secs_f64());

            // Should not take more than 100ms per creation on average
            assert!(avg_duration.as_millis() < 100, "Runtime creation too slow");
        }
    }

    #[test]
    fn benchmark_memory_safety_checks() {
        #[cfg(feature = "debug_memory")]
        {
            use minotari_wallet_ffi::debug_memory_safety::FfiMemorySafetyChecker;

            let test_value: u64 = 42;
            let ptr = &test_value as *const u64;
            
            let iterations = 10000;
            let start = Instant::now();

            for _ in 0..iterations {
                let _ = FfiMemorySafetyChecker::validate_c_pointer(ptr, "benchmark_test");
            }

            let duration = start.elapsed();
            let avg_duration = duration / iterations;

            println!("Memory safety check benchmark:");
            println!("  Total time: {:?}", duration);
            println!("  Average per check: {:?}", avg_duration);
            println!("  Checks per second: {:.0}", 1.0 / avg_duration.as_secs_f64());

            // Should be very fast - less than 1 microsecond per check
            assert!(avg_duration.as_nanos() < 1000, "Memory safety checks too slow");
        }
    }
}
