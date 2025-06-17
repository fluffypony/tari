// Tokio Runtime Conflict Testing
// Specialized tests for diagnosing Tokio runtime conflicts with Node.js

use std::sync::Arc;
use std::thread;
use std::time::Duration;
use tokio::runtime::{Runtime, Builder, Handle};
use log::{debug, error, warn, info};

#[derive(Debug, Clone)]
pub enum RuntimeStrategy {
    /// Standard multi-threaded runtime (current implementation)
    MultiThreaded,
    /// Single-threaded runtime to avoid thread conflicts
    SingleThreaded,
    /// Current thread runtime (requires existing runtime)
    CurrentThread,
    /// Dedicated thread runtime (isolated from Node.js)
    DedicatedThread,
}

impl RuntimeStrategy {
    pub fn as_str(&self) -> &'static str {
        match self {
            RuntimeStrategy::MultiThreaded => "multi_threaded",
            RuntimeStrategy::SingleThreaded => "single_threaded", 
            RuntimeStrategy::CurrentThread => "current_thread",
            RuntimeStrategy::DedicatedThread => "dedicated_thread",
        }
    }
}

pub struct RuntimeTester {
    strategy: RuntimeStrategy,
    test_results: Vec<RuntimeTestResult>,
}

#[derive(Debug, Clone)]
pub struct RuntimeTestResult {
    pub strategy: RuntimeStrategy,
    pub success: bool,
    pub error_message: Option<String>,
    pub duration_ms: u64,
    pub thread_id: thread::ThreadId,
}

impl RuntimeTester {
    pub fn new(strategy: RuntimeStrategy) -> Self {
        Self {
            strategy,
            test_results: Vec::new(),
        }
    }

    /// Test runtime creation with different strategies
    pub fn test_runtime_creation(&mut self) -> RuntimeTestResult {
        let start_time = std::time::Instant::now();
        let thread_id = thread::current().id();

        info!(
            target: "tari::wallet_ffi::debug::runtime_test",
            "Testing runtime strategy: {:?} on thread {:?}",
            self.strategy, thread_id
        );

        let result = match self.strategy {
            RuntimeStrategy::MultiThreaded => self.test_multi_threaded_runtime(),
            RuntimeStrategy::SingleThreaded => self.test_single_threaded_runtime(),
            RuntimeStrategy::CurrentThread => self.test_current_thread_runtime(),
            RuntimeStrategy::DedicatedThread => self.test_dedicated_thread_runtime(),
        };

        let duration_ms = start_time.elapsed().as_millis() as u64;
        
        let test_result = RuntimeTestResult {
            strategy: self.strategy.clone(),
            success: result.is_ok(),
            error_message: result.err(),
            duration_ms,
            thread_id,
        };

        self.test_results.push(test_result.clone());
        test_result
    }

    fn test_multi_threaded_runtime(&self) -> Result<(), String> {
        // This is the current implementation approach
        match Runtime::new() {
            Ok(runtime) => {
                debug!(target: "tari::wallet_ffi::debug::runtime_test", "Multi-threaded runtime created successfully");
                
                // Test basic async operation
                let result = runtime.block_on(async {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                    42
                });

                if result == 42 {
                    debug!(target: "tari::wallet_ffi::debug::runtime_test", "Multi-threaded runtime test passed");
                    Ok(())
                } else {
                    Err("Multi-threaded runtime test failed".to_string())
                }
            }
            Err(e) => {
                error!(target: "tari::wallet_ffi::debug::runtime_test", "Multi-threaded runtime creation failed: {}", e);
                Err(format!("Multi-threaded runtime creation failed: {}", e))
            }
        }
    }

    fn test_single_threaded_runtime(&self) -> Result<(), String> {
        // Test single-threaded runtime approach
        match Builder::new_current_thread().enable_all().build() {
            Ok(runtime) => {
                debug!(target: "tari::wallet_ffi::debug::runtime_test", "Single-threaded runtime created successfully");
                
                // Test basic async operation
                let result = runtime.block_on(async {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                    42
                });

                if result == 42 {
                    debug!(target: "tari::wallet_ffi::debug::runtime_test", "Single-threaded runtime test passed");
                    Ok(())
                } else {
                    Err("Single-threaded runtime test failed".to_string())
                }
            }
            Err(e) => {
                error!(target: "tari::wallet_ffi::debug::runtime_test", "Single-threaded runtime creation failed: {}", e);
                Err(format!("Single-threaded runtime creation failed: {}", e))
            }
        }
    }

    fn test_current_thread_runtime(&self) -> Result<(), String> {
        // Test using existing runtime handle
        match Handle::try_current() {
            Ok(handle) => {
                debug!(target: "tari::wallet_ffi::debug::runtime_test", "Current thread runtime handle obtained");
                
                // Test spawning on current runtime
                let join_handle = handle.spawn(async {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                    42
                });

                // We can't easily block_on here since we're already in a runtime
                // This would typically be used differently in production
                drop(join_handle);
                Ok(())
            }
            Err(e) => {
                warn!(target: "tari::wallet_ffi::debug::runtime_test", "No current runtime available: {}", e);
                Err(format!("No current runtime available: {}", e))
            }
        }
    }

    fn test_dedicated_thread_runtime(&self) -> Result<(), String> {
        // Test creating runtime on dedicated thread
        let (tx, rx) = std::sync::mpsc::channel();
        
        let handle = thread::spawn(move || {
            debug!(target: "tari::wallet_ffi::debug::runtime_test", "Creating runtime on dedicated thread");
            
            match Runtime::new() {
                Ok(runtime) => {
                    debug!(target: "tari::wallet_ffi::debug::runtime_test", "Dedicated thread runtime created");
                    
                    let result = runtime.block_on(async {
                        tokio::time::sleep(Duration::from_millis(10)).await;
                        42
                    });
                    
                    tx.send(Ok(result)).unwrap();
                }
                Err(e) => {
                    error!(target: "tari::wallet_ffi::debug::runtime_test", "Dedicated thread runtime creation failed: {}", e);
                    tx.send(Err(format!("Dedicated thread runtime creation failed: {}", e))).unwrap();
                }
            }
        });

        match rx.recv_timeout(Duration::from_secs(5)) {
            Ok(Ok(result)) => {
                if result == 42 {
                    debug!(target: "tari::wallet_ffi::debug::runtime_test", "Dedicated thread runtime test passed");
                    handle.join().map_err(|_| "Thread join failed".to_string())?;
                    Ok(())
                } else {
                    Err("Dedicated thread runtime test failed".to_string())
                }
            }
            Ok(Err(e)) => Err(e),
            Err(_) => {
                error!(target: "tari::wallet_ffi::debug::runtime_test", "Dedicated thread runtime test timed out");
                Err("Dedicated thread runtime test timed out".to_string())
            }
        }
    }

    pub fn get_test_results(&self) -> &[RuntimeTestResult] {
        &self.test_results
    }

    pub fn generate_report(&self) -> String {
        let mut report = String::new();
        report.push_str("=== Tokio Runtime Strategy Test Report ===\n");
        
        for result in &self.test_results {
            report.push_str(&format!(
                "Strategy: {:?}\n  Success: {}\n  Duration: {}ms\n  Thread: {:?}\n",
                result.strategy, result.success, result.duration_ms, result.thread_id
            ));
            
            if let Some(error) = &result.error_message {
                report.push_str(&format!("  Error: {}\n", error));
            }
            report.push('\n');
        }
        
        report
    }
}

/// Comprehensive runtime strategy testing
pub fn test_all_runtime_strategies() -> Vec<RuntimeTestResult> {
    let strategies = vec![
        RuntimeStrategy::MultiThreaded,
        RuntimeStrategy::SingleThreaded,
        RuntimeStrategy::CurrentThread,
        RuntimeStrategy::DedicatedThread,
    ];
    
    let mut all_results = Vec::new();
    
    for strategy in strategies {
        let mut tester = RuntimeTester::new(strategy);
        let result = tester.test_runtime_creation();
        all_results.push(result);
    }
    
    all_results
}

/// Simulate wallet_create runtime usage pattern
pub fn simulate_wallet_create_runtime_pattern() -> Result<(), String> {
    info!(target: "tari::wallet_ffi::debug::runtime_test", "Simulating wallet_create runtime pattern");
    
    // Simulate the exact pattern used in wallet_create
    let runtime = Runtime::new().map_err(|e| format!("Runtime creation failed: {}", e))?;
    
    // Simulate the database initialization pattern
    let db_result = runtime.block_on(async {
        debug!(target: "tari::wallet_ffi::debug::runtime_test", "Simulating database initialization");
        tokio::time::sleep(Duration::from_millis(100)).await;
        Ok::<_, String>("database_initialized".to_string())
    })?;
    
    debug!(target: "tari::wallet_ffi::debug::runtime_test", "Database simulation result: {}", db_result);
    
    // Simulate the wallet start pattern
    let wallet_result = runtime.block_on(async {
        debug!(target: "tari::wallet_ffi::debug::runtime_test", "Simulating wallet start");
        tokio::time::sleep(Duration::from_millis(200)).await;
        Ok::<_, String>("wallet_started".to_string())
    })?;
    
    debug!(target: "tari::wallet_ffi::debug::runtime_test", "Wallet simulation result: {}", wallet_result);
    
    // Simulate the callback handler spawn pattern
    let _callback_handle = runtime.spawn(async {
        debug!(target: "tari::wallet_ffi::debug::runtime_test", "Simulating callback handler");
        
        // Simulate long-running callback handler
        loop {
            tokio::time::sleep(Duration::from_millis(1000)).await;
            debug!(target: "tari::wallet_ffi::debug::runtime_test", "Callback handler tick");
        }
    });
    
    info!(target: "tari::wallet_ffi::debug::runtime_test", "Wallet create simulation completed successfully");
    
    // In real implementation, runtime would be stored in TariWallet struct
    // Here we just drop it to complete the test
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_runtime_strategy_multi_threaded() {
        let mut tester = RuntimeTester::new(RuntimeStrategy::MultiThreaded);
        let result = tester.test_runtime_creation();
        assert!(result.success, "Multi-threaded runtime should work in test environment");
    }

    #[test]
    fn test_runtime_strategy_single_threaded() {
        let mut tester = RuntimeTester::new(RuntimeStrategy::SingleThreaded);
        let result = tester.test_runtime_creation();
        assert!(result.success, "Single-threaded runtime should work in test environment");
    }

    #[tokio::test]
    async fn test_runtime_strategy_current_thread() {
        let mut tester = RuntimeTester::new(RuntimeStrategy::CurrentThread);
        let result = tester.test_runtime_creation();
        assert!(result.success, "Current thread runtime should work when in Tokio context");
    }

    #[test]
    fn test_runtime_strategy_dedicated_thread() {
        let mut tester = RuntimeTester::new(RuntimeStrategy::DedicatedThread);
        let result = tester.test_runtime_creation();
        assert!(result.success, "Dedicated thread runtime should work in test environment");
    }

    #[test]
    fn test_all_strategies() {
        let results = test_all_runtime_strategies();
        assert!(!results.is_empty());
        
        // At least some strategies should work in test environment
        let successful_strategies = results.iter().filter(|r| r.success).count();
        assert!(successful_strategies > 0, "At least one runtime strategy should work");
    }

    #[test]
    fn test_wallet_create_simulation() {
        let result = simulate_wallet_create_runtime_pattern();
        assert!(result.is_ok(), "Wallet create simulation should succeed");
    }
}
