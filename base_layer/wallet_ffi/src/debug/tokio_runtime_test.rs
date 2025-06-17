//! Tokio Runtime Testing and Analysis
//! 
//! Comprehensive testing toolkit for analyzing Tokio runtime behavior in Node.js environments
//! and identifying potential conflicts that cause segfaults.

use std::{
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};
use tokio::runtime::{Builder, Handle, Runtime};
use log::{debug, error, info, warn};

/// Runtime strategy enumeration for testing different approaches
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RuntimeStrategy {
    /// Multi-threaded runtime (default Tokio behavior)
    MultiThreaded,
    /// Single-threaded current thread runtime
    CurrentThread,
    /// Dedicated thread with its own runtime
    DedicatedThread,
    /// Use existing runtime handle if available
    UseExisting,
}

/// Runtime test results
#[derive(Debug)]
pub struct RuntimeTestResult {
    pub strategy: RuntimeStrategy,
    pub success: bool,
    pub duration: Duration,
    pub error: Option<String>,
    pub nodejs_detected: bool,
    pub event_loop_conflicts: bool,
}

/// Tokio runtime testing framework
pub struct TokioRuntimeTester {
    results: Arc<Mutex<Vec<RuntimeTestResult>>>,
}

impl TokioRuntimeTester {
    /// Create new runtime tester
    pub fn new() -> Self {
        Self {
            results: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Test all runtime strategies
    pub async fn test_all_strategies(&self) -> Vec<RuntimeTestResult> {
        let strategies = [
            RuntimeStrategy::MultiThreaded,
            RuntimeStrategy::CurrentThread,
            RuntimeStrategy::DedicatedThread,
            RuntimeStrategy::UseExisting,
        ];

        let mut results = Vec::new();
        
        for &strategy in &strategies {
            info!("Testing runtime strategy: {:?}", strategy);
            let result = self.test_strategy(strategy).await;
            results.push(result);
        }

        *self.results.lock().unwrap() = results.clone();
        results
    }

    /// Test a specific runtime strategy
    pub async fn test_strategy(&self, strategy: RuntimeStrategy) -> RuntimeTestResult {
        let start_time = Instant::now();
        let nodejs_detected = self.detect_nodejs_environment();
        
        info!("Testing strategy {:?} (Node.js detected: {})", strategy, nodejs_detected);

        let (success, error, event_loop_conflicts) = match strategy {
            RuntimeStrategy::MultiThreaded => self.test_multithreaded_runtime().await,
            RuntimeStrategy::CurrentThread => self.test_current_thread_runtime().await,
            RuntimeStrategy::DedicatedThread => self.test_dedicated_thread_runtime().await,
            RuntimeStrategy::UseExisting => self.test_existing_runtime().await,
        };

        RuntimeTestResult {
            strategy,
            success,
            duration: start_time.elapsed(),
            error,
            nodejs_detected,
            event_loop_conflicts,
        }
    }

    /// Test multi-threaded runtime
    async fn test_multithreaded_runtime(&self) -> (bool, Option<String>, bool) {
        match Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
        {
            Ok(rt) => {
                let conflicts = self.check_event_loop_conflicts(&rt).await;
                let test_success = self.run_basic_async_test(&rt).await;
                (test_success, None, conflicts)
            }
            Err(e) => (false, Some(e.to_string()), false),
        }
    }

    /// Test current thread runtime
    async fn test_current_thread_runtime(&self) -> (bool, Option<String>, bool) {
        match Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(rt) => {
                let conflicts = self.check_event_loop_conflicts(&rt).await;
                let test_success = self.run_basic_async_test(&rt).await;
                (test_success, None, conflicts)
            }
            Err(e) => (false, Some(e.to_string()), false),
        }
    }

    /// Test dedicated thread runtime
    async fn test_dedicated_thread_runtime(&self) -> (bool, Option<String>, bool) {
        let (tx, rx) = std::sync::mpsc::channel();
        
        thread::spawn(move || {
            let rt = match Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(e) => {
                    let _ = tx.send((false, Some(e.to_string()), false));
                    return;
                }
            };

            // Run test in dedicated thread
            let test_result = rt.block_on(async {
                // Basic async operation test
                tokio::time::sleep(Duration::from_millis(10)).await;
                true
            });

            let _ = tx.send((test_result, None, false)); // No event loop conflicts in dedicated thread
        });

        match rx.recv_timeout(Duration::from_secs(5)) {
            Ok(result) => result,
            Err(_) => (false, Some("Dedicated thread test timeout".to_string()), false),
        }
    }

    /// Test using existing runtime handle
    async fn test_existing_runtime(&self) -> (bool, Option<String>, bool) {
        match Handle::try_current() {
            Ok(handle) => {
                info!("Using existing Tokio runtime handle");
                
                // Spawn a test task
                let test_result = handle.spawn(async {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                    true
                }).await;

                match test_result {
                    Ok(success) => (success, None, false),
                    Err(e) => (false, Some(e.to_string()), false),
                }
            }
            Err(_) => (false, Some("No existing runtime handle available".to_string()), false),
        }
    }

    /// Run basic async test on runtime
    async fn run_basic_async_test(&self, rt: &Runtime) -> bool {
        let handle = rt.handle();
        
        match handle.spawn(async {
            // Test basic async operations
            tokio::time::sleep(Duration::from_millis(10)).await;
            
            // Test channel operations
            let (tx, mut rx) = tokio::sync::mpsc::channel(1);
            tx.send("test").await.unwrap();
            let received = rx.recv().await.unwrap();
            
            received == "test"
        }).await {
            Ok(result) => result,
            Err(e) => {
                error!("Async test failed: {}", e);
                false
            }
        }
    }

    /// Check for event loop conflicts
    async fn check_event_loop_conflicts(&self, rt: &Runtime) -> bool {
        let handle = rt.handle();
        
        // Test for common conflict patterns
        let mut conflicts = false;

        // Test 1: Rapid task spawning (simulates Node.js callback pressure)
        let spawn_test = handle.spawn(async {
            for i in 0..100 {
                tokio::task::yield_now().await;
                if i % 10 == 0 {
                    tokio::time::sleep(Duration::from_micros(1)).await;
                }
            }
        }).await;

        if spawn_test.is_err() {
            warn!("Rapid task spawning test failed - potential event loop conflict");
            conflicts = true;
        }

        // Test 2: Timer precision (Node.js can interfere with timer accuracy)
        let timer_start = Instant::now();
        let timer_test = handle.spawn(async {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }).await;

        let timer_duration = timer_start.elapsed();
        if timer_test.is_err() || timer_duration > Duration::from_millis(100) {
            warn!("Timer test failed or imprecise - potential event loop conflict");
            conflicts = true;
        }

        // Test 3: Channel operations under load
        let channel_test = handle.spawn(async {
            let (tx, mut rx) = tokio::sync::mpsc::channel(10);
            
            for i in 0..50 {
                if tx.send(i).await.is_err() {
                    return false;
                }
            }
            
            for _ in 0..50 {
                if rx.recv().await.is_none() {
                    return false;
                }
            }
            
            true
        }).await;

        if channel_test.unwrap_or(false) == false {
            warn!("Channel test failed - potential event loop conflict");
            conflicts = true;
        }

        conflicts
    }

    /// Detect Node.js environment
    fn detect_nodejs_environment(&self) -> bool {
        // Check for Node.js environment variables
        if std::env::var("NODE_VERSION").is_ok() || 
           std::env::var("npm_config_registry").is_ok() ||
           std::env::var("NODE_ENV").is_ok() {
            return true;
        }

        // Check for process name patterns
        if let Ok(exe) = std::env::current_exe() {
            if let Some(name) = exe.file_name() {
                if let Some(name_str) = name.to_str() {
                    return name_str.contains("node") || name_str.contains("npm");
                }
            }
        }

        false
    }

    /// Get test results
    pub fn get_results(&self) -> Vec<RuntimeTestResult> {
        self.results.lock().unwrap().clone()
    }

    /// Find best strategy for current environment
    pub fn find_best_strategy(&self) -> Option<RuntimeStrategy> {
        let results = self.get_results();
        
        // Prefer strategies that work without conflicts
        for result in &results {
            if result.success && !result.event_loop_conflicts {
                return Some(result.strategy);
            }
        }

        // Fallback to any working strategy
        for result in &results {
            if result.success {
                return Some(result.strategy);
            }
        }

        None
    }

    /// Generate diagnostic report
    pub fn generate_report(&self) -> String {
        let results = self.get_results();
        let nodejs_env = self.detect_nodejs_environment();
        
        let mut report = String::new();
        report.push_str("=== TOKIO RUNTIME COMPATIBILITY REPORT ===\n");
        report.push_str(&format!("Node.js Environment Detected: {}\n", nodejs_env));
        report.push_str(&format!("Strategies Tested: {}\n\n", results.len()));

        for result in &results {
            report.push_str(&format!("Strategy: {:?}\n", result.strategy));
            report.push_str(&format!("  Success: {}\n", result.success));
            report.push_str(&format!("  Duration: {:?}\n", result.duration));
            report.push_str(&format!("  Event Loop Conflicts: {}\n", result.event_loop_conflicts));
            if let Some(ref error) = result.error {
                report.push_str(&format!("  Error: {}\n", error));
            }
            report.push('\n');
        }

        if let Some(best) = self.find_best_strategy() {
            report.push_str(&format!("Recommended Strategy: {:?}\n", best));
        } else {
            report.push_str("No compatible strategy found!\n");
        }

        report
    }
}

/// Run comprehensive runtime compatibility test
pub async fn run_compatibility_test() -> RuntimeTestResult {
    let tester = TokioRuntimeTester::new();
    let results = tester.test_all_strategies().await;
    
    info!("{}", tester.generate_report());
    
    // Return best result or first successful one
    results.into_iter()
        .find(|r| r.success && !r.event_loop_conflicts)
        .or_else(|| results.into_iter().find(|r| r.success))
        .unwrap_or(RuntimeTestResult {
            strategy: RuntimeStrategy::MultiThreaded,
            success: false,
            duration: Duration::from_secs(0),
            error: Some("All strategies failed".to_string()),
            nodejs_detected: tester.detect_nodejs_environment(),
            event_loop_conflicts: true,
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::debug::segfault_investigation::init_test_debugging;

    #[tokio::test]
    async fn test_runtime_strategies() {
        init_test_debugging();
        
        let tester = TokioRuntimeTester::new();
        let results = tester.test_all_strategies().await;
        
        assert!(!results.is_empty());
        assert!(results.iter().any(|r| r.success));
    }

    #[tokio::test]
    async fn test_compatibility() {
        init_test_debugging();
        
        let result = run_compatibility_test().await;
        
        // Should at least attempt to run
        assert!(result.duration > Duration::from_nanos(1));
    }
}
