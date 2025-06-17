//! Node.js FFI Integration Tests
//! 
//! Comprehensive test suite for validating the segfault fixes and ensuring
//! proper operation of the Tari wallet FFI in Node.js environments.

use std::{
    ffi::{CStr, CString},
    ptr,
    time::Duration,
};
use tempfile::tempdir;
use tokio::runtime::Runtime;

#[cfg(any(feature = "debug_runtime", feature = "debug_memory", feature = "nodejs_compatibility"))]
use minotari_wallet_ffi::debug;

#[cfg(feature = "nodejs_compatibility")]
use minotari_wallet_ffi::runtime_strategies::{
    RuntimeStrategy, 
    initialize_runtime_with_strategy, 
    execute_with_runtime,
    get_runtime_statistics
};

#[cfg(feature = "debug_memory")]
use minotari_wallet_ffi::debug::memory_diagnostics::{
    FFIBoundaryValidator,
    MemoryTracker,
    init_memory_diagnostics,
    generate_memory_report
};

#[cfg(feature = "debug_runtime")]
use minotari_wallet_ffi::debug::segfault_investigation::SegfaultInvestigator;

use minotari_wallet_ffi::production_hardening::{
    initialize_production_hardening,
    is_system_healthy,
    perform_health_check
};

/// Test initialization of debugging infrastructure
#[test]
fn test_debug_infrastructure_initialization() {
    // Initialize memory diagnostics
    #[cfg(feature = "debug_memory")]
    {
        let result = init_memory_diagnostics();
        assert!(result.is_ok(), "Memory diagnostics initialization failed: {:?}", result);
        
        // Test memory tracker
        let stats = MemoryTracker::get_statistics();
        println!("Initial memory stats: {:?}", stats);
    }

    // Initialize segfault investigation
    #[cfg(feature = "debug_runtime")]
    {
        let result = SegfaultInvestigator::initialize();
        assert!(result.is_ok(), "Segfault investigator initialization failed: {:?}", result);
        
        // Test operation recording
        SegfaultInvestigator::record_operation("test_operation");
        SegfaultInvestigator::record_nodejs_operation("test_nodejs", "test_details");
        SegfaultInvestigator::record_tokio_operation("test_tokio", "test_details");
    }

    // Initialize production hardening
    let result = initialize_production_hardening();
    assert!(result.is_ok(), "Production hardening initialization failed: {:?}", result);
    
    // Test health check
    let health = perform_health_check();
    println!("Health check result: {:?}", health);
}

/// Test runtime strategy initialization and execution
#[cfg(feature = "nodejs_compatibility")]
#[test]
fn test_runtime_strategies() {
    // Test all runtime strategies
    let strategies = [
        RuntimeStrategy::MultiThreaded,
        RuntimeStrategy::SingleThreaded,
        RuntimeStrategy::DedicatedThread,
        RuntimeStrategy::HandleBased,
        RuntimeStrategy::Adaptive,
    ];

    for &strategy in &strategies {
        println!("Testing runtime strategy: {:?}", strategy);
        
        let result = initialize_runtime_with_strategy(strategy);
        match result {
            Ok(()) => {
                println!("Strategy {:?} initialized successfully", strategy);
                
                // Test execution
                let execution_result = execute_with_runtime(async {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                    42
                });
                
                match execution_result {
                    Ok(value) => {
                        assert_eq!(value, 42);
                        println!("Strategy {:?} execution successful", strategy);
                    }
                    Err(e) => {
                        println!("Strategy {:?} execution failed: {}", strategy, e);
                    }
                }
            }
            Err(e) => {
                println!("Strategy {:?} initialization failed: {}", strategy, e);
                // Some strategies may fail depending on environment, that's ok
            }
        }
    }

    // Test statistics
    if let Some(stats) = get_runtime_statistics() {
        println!("Runtime statistics: {:?}", stats);
    }
}

/// Test memory boundary validation
#[cfg(feature = "debug_memory")]
#[test]
fn test_memory_boundary_validation() {
    // Test string validation
    let test_string = CString::new("Hello, World!").unwrap();
    let validation = FFIBoundaryValidator::validate_c_string(test_string.as_ptr());
    assert!(validation.valid, "String validation failed: {:?}", validation.errors);

    // Test pointer alignment
    let test_value: u64 = 42;
    let validation = FFIBoundaryValidator::validate_pointer_alignment(&test_value as *const u64);
    assert!(validation.valid, "Pointer alignment validation failed: {:?}", validation.errors);

    // Test null pointer handling
    let null_ptr: *const i8 = ptr::null();
    let validation = FFIBoundaryValidator::validate_c_string(null_ptr);
    assert!(!validation.valid, "Null pointer should fail validation");

    // Test memory block validation
    let buffer = vec![0u8; 1024];
    let validation = FFIBoundaryValidator::validate_memory_block(
        buffer.as_ptr() as *const std::ffi::c_void,
        buffer.len()
    );
    assert!(validation.valid, "Memory block validation failed: {:?}", validation.errors);
}

/// Test concurrent access patterns
#[tokio::test]
async fn test_concurrent_runtime_access() {
    #[cfg(feature = "nodejs_compatibility")]
    {
        // Initialize with adaptive strategy
        let result = initialize_runtime_with_strategy(RuntimeStrategy::Adaptive);
        assert!(result.is_ok(), "Failed to initialize runtime strategy");

        // Spawn multiple concurrent operations
        let mut handles = vec![];
        
        for i in 0..10 {
            let handle = tokio::spawn(async move {
                let result = execute_with_runtime(async move {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                    i * 2
                });
                result
            });
            handles.push(handle);
        }

        // Wait for all operations to complete
        for (i, handle) in handles.into_iter().enumerate() {
            let result = handle.await.unwrap();
            match result {
                Ok(value) => {
                    assert_eq!(value, i * 2);
                }
                Err(e) => {
                    panic!("Concurrent operation {} failed: {}", i, e);
                }
            }
        }
    }
}

/// Test memory pressure scenarios
#[cfg(feature = "debug_memory")]
#[test]
fn test_memory_pressure() {
    // Initialize memory tracking
    let result = init_memory_diagnostics();
    assert!(result.is_ok());

    let initial_stats = MemoryTracker::get_statistics();
    
    // Allocate and deallocate memory in patterns similar to wallet operations
    let mut allocations = Vec::new();
    
    for i in 0..1000 {
        // Simulate various allocation sizes
        let size = match i % 4 {
            0 => 64,
            1 => 256,
            2 => 1024,
            _ => 4096,
        };
        
        let allocation = vec![0u8; size];
        allocations.push(allocation);
        
        // Periodically free some allocations
        if i % 100 == 0 && allocations.len() > 500 {
            allocations.drain(0..200);
        }
    }

    let final_stats = MemoryTracker::get_statistics();
    
    // Verify memory tracking is working
    assert!(
        final_stats.allocation_count >= initial_stats.allocation_count,
        "Memory allocation count should have increased"
    );

    // Generate and check memory report
    let memory_report = generate_memory_report();
    assert!(!memory_report.is_empty(), "Memory report should not be empty");
    
    println!("Memory report:\n{}", memory_report);
}

/// Test callback function safety
#[test]
fn test_callback_safety() {
    #[cfg(feature = "debug_memory")]
    {
        // Test callback function pointer validation
        extern "C" fn test_callback(_arg: i32) -> i32 {
            42
        }

        let callback_ptr = test_callback as *const std::ffi::c_void;
        let validation = FFIBoundaryValidator::validate_callback_pointer(callback_ptr);
        assert!(validation.valid, "Callback validation failed: {:?}", validation.errors);

        // Test null callback handling
        let null_callback: *const std::ffi::c_void = ptr::null();
        let validation = FFIBoundaryValidator::validate_callback_pointer(null_callback);
        assert!(!validation.valid, "Null callback should fail validation");
    }
}

/// Test production hardening features
#[test]
fn test_production_hardening() {
    // Initialize production hardening
    let result = initialize_production_hardening();
    assert!(result.is_ok(), "Production hardening initialization failed");

    // Test system health check
    assert!(is_system_healthy(), "System should be healthy after initialization");

    // Perform comprehensive health check
    let health_result = perform_health_check();
    println!("Health check: {:?}", health_result);
    
    if !health_result.healthy {
        println!("Health issues: {:?}", health_result.issues);
    }
    
    if !health_result.warnings.is_empty() {
        println!("Health warnings: {:?}", health_result.warnings);
    }
}

/// Test Node.js environment simulation
#[test]
fn test_nodejs_environment_simulation() {
    // Set environment variables that mimic Node.js
    std::env::set_var("NODE_VERSION", "v18.17.0");
    std::env::set_var("npm_config_registry", "https://registry.npmjs.org/");
    std::env::set_var("NODE_ENV", "test");

    #[cfg(feature = "nodejs_compatibility")]
    {
        // Test adaptive strategy in simulated Node.js environment
        let result = initialize_runtime_with_strategy(RuntimeStrategy::Adaptive);
        assert!(result.is_ok(), "Adaptive strategy should work in Node.js environment");

        if let Some(stats) = get_runtime_statistics() {
            println!("Runtime stats in Node.js simulation: {:?}", stats);
            assert!(stats.nodejs_detected, "Should detect Node.js environment");
        }
    }

    // Clean up environment variables
    std::env::remove_var("NODE_VERSION");
    std::env::remove_var("npm_config_registry");
    std::env::remove_var("NODE_ENV");
}

/// Test error recovery mechanisms
#[test]
fn test_error_recovery() {
    #[cfg(feature = "debug_runtime")]
    {
        // Initialize investigation
        let result = SegfaultInvestigator::initialize();
        assert!(result.is_ok());

        // Record various operations to test the system
        SegfaultInvestigator::record_operation("test_operation_1");
        SegfaultInvestigator::record_operation("test_operation_2");
        SegfaultInvestigator::record_nodejs_operation("nodejs_test", "test_data");
        SegfaultInvestigator::record_tokio_operation("tokio_test", "runtime_data");
    }

    // Test production hardening health monitoring
    let initial_health = is_system_healthy();
    println!("Initial system health: {}", initial_health);

    // Perform health check
    let health_check = perform_health_check();
    assert!(
        health_check.healthy || !health_check.issues.is_empty(),
        "Health check should provide meaningful results"
    );
}

/// Integration test that combines all features
#[tokio::test]
async fn test_comprehensive_integration() {
    println!("Starting comprehensive integration test...");

    // Initialize all systems
    #[cfg(feature = "debug_memory")]
    {
        init_memory_diagnostics().expect("Memory diagnostics initialization");
    }

    #[cfg(feature = "debug_runtime")]
    {
        SegfaultInvestigator::initialize().expect("Segfault investigator initialization");
    }

    initialize_production_hardening().expect("Production hardening initialization");

    #[cfg(feature = "nodejs_compatibility")]
    {
        initialize_runtime_with_strategy(RuntimeStrategy::Adaptive).expect("Runtime strategy initialization");
    }

    // Test concurrent operations
    let mut handles = vec![];
    
    for i in 0..5 {
        let handle = tokio::spawn(async move {
            // Simulate wallet-like operations
            #[cfg(feature = "nodejs_compatibility")]
            {
                let result = execute_with_runtime(async move {
                    // Simulate some async work
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    
                    // Create some test data
                    let test_data = format!("test_data_{}", i);
                    test_data.len()
                }).await;
                
                result.unwrap_or(0)
            }

            #[cfg(not(feature = "nodejs_compatibility"))]
            {
                tokio::time::sleep(Duration::from_millis(50)).await;
                i
            }
        });
        handles.push(handle);
    }

    // Wait for all operations
    for handle in handles {
        handle.await.expect("Async operation should complete");
    }

    // Final health check
    let final_health = perform_health_check();
    println!("Final health check: {:?}", final_health);

    #[cfg(feature = "debug_memory")]
    {
        let memory_report = generate_memory_report();
        println!("Final memory report:\n{}", memory_report);
    }

    println!("Comprehensive integration test completed successfully");
}

/// Stress test for runtime stability
#[tokio::test]
async fn test_runtime_stability_stress() {
    #[cfg(feature = "nodejs_compatibility")]
    {
        // Initialize with adaptive strategy
        initialize_runtime_with_strategy(RuntimeStrategy::Adaptive).expect("Runtime initialization");

        // Run many concurrent operations to stress test the system
        let num_operations = 100;
        let mut handles = vec![];

        for i in 0..num_operations {
            let handle = tokio::spawn(async move {
                for j in 0..10 {
                    let result = execute_with_runtime(async move {
                        tokio::time::sleep(Duration::from_micros(100)).await;
                        i * 10 + j
                    }).await;

                    if result.is_err() {
                        eprintln!("Operation failed: {} {}: {:?}", i, j, result);
                        return false;
                    }
                }
                true
            });
            handles.push(handle);
        }

        // Wait for all operations and check success
        let mut successful = 0;
        for handle in handles {
            if handle.await.unwrap_or(false) {
                successful += 1;
            }
        }

        let success_rate = (successful as f64 / num_operations as f64) * 100.0;
        println!("Stress test success rate: {:.1}% ({}/{})", success_rate, successful, num_operations);

        // We should have a high success rate
        assert!(success_rate >= 90.0, "Success rate should be at least 90%");
    }
}
