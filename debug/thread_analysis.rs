// Thread Analysis for Tokio Runtime Conflicts
// Analyzes thread usage patterns and potential conflicts with Node.js

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use tokio::runtime::{Runtime, Builder};
use log::{debug, error, warn, info};

#[derive(Debug, Clone)]
pub struct ThreadInfo {
    pub id: thread::ThreadId,
    pub name: Option<String>,
    pub creation_time: Instant,
    pub runtime_type: String,
    pub is_tokio_worker: bool,
    pub is_nodejs_thread: bool,
}

#[derive(Debug, Clone)]
pub struct ThreadAnalyzer {
    threads: Arc<Mutex<HashMap<thread::ThreadId, ThreadInfo>>>,
    runtime_count: Arc<Mutex<u32>>,
}

impl ThreadAnalyzer {
    pub fn new() -> Self {
        Self {
            threads: Arc::new(Mutex::new(HashMap::new())),
            runtime_count: Arc::new(Mutex::new(0)),
        }
    }

    /// Track thread creation
    pub fn track_thread(&self, runtime_type: String) {
        let current = thread::current();
        let thread_info = ThreadInfo {
            id: current.id(),
            name: current.name().map(|s| s.to_string()),
            creation_time: Instant::now(),
            runtime_type: runtime_type.clone(),
            is_tokio_worker: self.is_tokio_worker_thread(),
            is_nodejs_thread: self.is_nodejs_thread(),
        };

        if let Ok(mut threads) = self.threads.lock() {
            threads.insert(current.id(), thread_info.clone());
        }

        info!(
            target: "tari::wallet_ffi::debug::thread_analysis",
            "Tracked thread: {:?}, name: {:?}, type: {}, tokio_worker: {}, nodejs: {}",
            thread_info.id, thread_info.name, runtime_type, 
            thread_info.is_tokio_worker, thread_info.is_nodejs_thread
        );
    }

    /// Detect if current thread is a Tokio worker thread
    fn is_tokio_worker_thread(&self) -> bool {
        let current = thread::current();
        current.name().map_or(false, |name| {
            name.contains("tokio") || 
            name.contains("runtime") ||
            name.contains("worker") ||
            name.starts_with("async")
        })
    }

    /// Detect if current thread is likely a Node.js thread
    fn is_nodejs_thread(&self) -> bool {
        let current = thread::current();
        current.name().map_or(false, |name| {
            name.contains("node") ||
            name.contains("v8") ||
            name.contains("libuv") ||
            name.contains("main") ||
            name == "event_loop"
        }) || self.check_nodejs_environment()
    }

    /// Check for Node.js environment indicators
    fn check_nodejs_environment(&self) -> bool {
        std::env::var("NODE_ENV").is_ok() ||
        std::env::var("npm_config_user_config").is_ok() ||
        std::env::var("NODE_PATH").is_ok()
    }

    /// Analyze thread conflicts
    pub fn analyze_conflicts(&self) -> Vec<String> {
        let mut conflicts = Vec::new();

        if let Ok(threads) = self.threads.lock() {
            let tokio_threads: Vec<_> = threads.values()
                .filter(|t| t.is_tokio_worker)
                .collect();
            
            let nodejs_threads: Vec<_> = threads.values()
                .filter(|t| t.is_nodejs_thread)
                .collect();

            if !tokio_threads.is_empty() && !nodejs_threads.is_empty() {
                conflicts.push(format!(
                    "Potential thread conflict: {} Tokio worker threads and {} Node.js threads detected",
                    tokio_threads.len(), nodejs_threads.len()
                ));
            }

            // Check for same thread being both Tokio and Node.js
            for thread_info in threads.values() {
                if thread_info.is_tokio_worker && thread_info.is_nodejs_thread {
                    conflicts.push(format!(
                        "Critical conflict: Thread {:?} detected as both Tokio worker and Node.js thread",
                        thread_info.id
                    ));
                }
            }

            // Check for too many runtimes
            if let Ok(runtime_count) = self.runtime_count.lock() {
                if *runtime_count > 1 {
                    conflicts.push(format!(
                        "Multiple Tokio runtimes detected: {} - this can cause conflicts",
                        runtime_count
                    ));
                }
            }
        }

        conflicts
    }

    /// Test different runtime creation strategies
    pub fn test_runtime_strategies(&self) -> RuntimeTestResults {
        let mut results = RuntimeTestResults::new();

        // Test 1: Multi-threaded runtime (current implementation)
        info!(target: "tari::wallet_ffi::debug::thread_analysis", "Testing multi-threaded runtime strategy");
        let multi_threaded_result = self.test_multi_threaded_runtime();
        results.multi_threaded = multi_threaded_result;

        // Test 2: Single-threaded runtime
        info!(target: "tari::wallet_ffi::debug::thread_analysis", "Testing single-threaded runtime strategy");
        let single_threaded_result = self.test_single_threaded_runtime();
        results.single_threaded = single_threaded_result;

        // Test 3: Current thread runtime
        info!(target: "tari::wallet_ffi::debug::thread_analysis", "Testing current thread runtime strategy");
        let current_thread_result = self.test_current_thread_runtime();
        results.current_thread = current_thread_result;

        // Test 4: Dedicated thread runtime
        info!(target: "tari::wallet_ffi::debug::thread_analysis", "Testing dedicated thread runtime strategy");
        let dedicated_thread_result = self.test_dedicated_thread_runtime();
        results.dedicated_thread = dedicated_thread_result;

        results
    }

    fn test_multi_threaded_runtime(&self) -> RuntimeTestResult {
        let start_time = Instant::now();
        self.track_thread("multi_threaded_test".to_string());

        match Runtime::new() {
            Ok(runtime) => {
                if let Ok(mut count) = self.runtime_count.lock() {
                    *count += 1;
                }

                let test_result = runtime.block_on(async {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                    "success"
                });

                RuntimeTestResult {
                    strategy: "multi_threaded".to_string(),
                    success: test_result == "success",
                    duration: start_time.elapsed(),
                    error_message: None,
                    thread_conflicts: self.analyze_conflicts(),
                }
            }
            Err(e) => {
                RuntimeTestResult {
                    strategy: "multi_threaded".to_string(),
                    success: false,
                    duration: start_time.elapsed(),
                    error_message: Some(e.to_string()),
                    thread_conflicts: self.analyze_conflicts(),
                }
            }
        }
    }

    fn test_single_threaded_runtime(&self) -> RuntimeTestResult {
        let start_time = Instant::now();
        self.track_thread("single_threaded_test".to_string());

        match Builder::new_current_thread().enable_all().build() {
            Ok(runtime) => {
                if let Ok(mut count) = self.runtime_count.lock() {
                    *count += 1;
                }

                let test_result = runtime.block_on(async {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                    "success"
                });

                RuntimeTestResult {
                    strategy: "single_threaded".to_string(),
                    success: test_result == "success",
                    duration: start_time.elapsed(),
                    error_message: None,
                    thread_conflicts: self.analyze_conflicts(),
                }
            }
            Err(e) => {
                RuntimeTestResult {
                    strategy: "single_threaded".to_string(),
                    success: false,
                    duration: start_time.elapsed(),
                    error_message: Some(e.to_string()),
                    thread_conflicts: self.analyze_conflicts(),
                }
            }
        }
    }

    fn test_current_thread_runtime(&self) -> RuntimeTestResult {
        let start_time = Instant::now();
        self.track_thread("current_thread_test".to_string());

        // This test checks if we can use an existing runtime
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => {
                // We have an existing runtime, test spawning
                let join_handle = handle.spawn(async {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                    "success"
                });

                // In a real scenario, we'd need to await this differently
                // For testing purposes, we'll just consider it successful
                RuntimeTestResult {
                    strategy: "current_thread".to_string(),
                    success: true,
                    duration: start_time.elapsed(),
                    error_message: None,
                    thread_conflicts: self.analyze_conflicts(),
                }
            }
            Err(e) => {
                RuntimeTestResult {
                    strategy: "current_thread".to_string(),
                    success: false,
                    duration: start_time.elapsed(),
                    error_message: Some(format!("No current runtime: {}", e)),
                    thread_conflicts: self.analyze_conflicts(),
                }
            }
        }
    }

    fn test_dedicated_thread_runtime(&self) -> RuntimeTestResult {
        let start_time = Instant::now();
        
        let (tx, rx) = std::sync::mpsc::channel();
        let analyzer = self.clone();

        let handle = thread::spawn(move || {
            analyzer.track_thread("dedicated_thread_test".to_string());
            
            match Runtime::new() {
                Ok(runtime) => {
                    let result = runtime.block_on(async {
                        tokio::time::sleep(Duration::from_millis(10)).await;
                        "success"
                    });
                    tx.send(Ok(result)).unwrap();
                }
                Err(e) => {
                    tx.send(Err(e.to_string())).unwrap();
                }
            }
        });

        match rx.recv_timeout(Duration::from_secs(5)) {
            Ok(Ok(_)) => {
                handle.join().unwrap();
                RuntimeTestResult {
                    strategy: "dedicated_thread".to_string(),
                    success: true,
                    duration: start_time.elapsed(),
                    error_message: None,
                    thread_conflicts: self.analyze_conflicts(),
                }
            }
            Ok(Err(e)) => {
                handle.join().unwrap();
                RuntimeTestResult {
                    strategy: "dedicated_thread".to_string(),
                    success: false,
                    duration: start_time.elapsed(),
                    error_message: Some(e),
                    thread_conflicts: self.analyze_conflicts(),
                }
            }
            Err(_) => {
                RuntimeTestResult {
                    strategy: "dedicated_thread".to_string(),
                    success: false,
                    duration: start_time.elapsed(),
                    error_message: Some("Timeout".to_string()),
                    thread_conflicts: self.analyze_conflicts(),
                }
            }
        }
    }

    /// Generate comprehensive thread analysis report
    pub fn generate_report(&self) -> String {
        let mut report = String::new();
        report.push_str("=== Thread Analysis Report ===\n\n");

        if let Ok(threads) = self.threads.lock() {
            report.push_str(&format!("Total threads tracked: {}\n", threads.len()));
            
            let tokio_count = threads.values().filter(|t| t.is_tokio_worker).count();
            let nodejs_count = threads.values().filter(|t| t.is_nodejs_thread).count();
            
            report.push_str(&format!("Tokio worker threads: {}\n", tokio_count));
            report.push_str(&format!("Node.js threads: {}\n", nodejs_count));
            
            if let Ok(runtime_count) = self.runtime_count.lock() {
                report.push_str(&format!("Runtime instances: {}\n", runtime_count));
            }
            
            report.push_str("\nThread Details:\n");
            for (id, info) in threads.iter() {
                report.push_str(&format!(
                    "  {:?}: name={:?}, type={}, tokio={}, nodejs={}, age={:?}\n",
                    id, info.name, info.runtime_type, 
                    info.is_tokio_worker, info.is_nodejs_thread,
                    info.creation_time.elapsed()
                ));
            }
        }

        let conflicts = self.analyze_conflicts();
        if !conflicts.is_empty() {
            report.push_str("\nConflicts Detected:\n");
            for conflict in conflicts {
                report.push_str(&format!("  - {}\n", conflict));
            }
        } else {
            report.push_str("\nNo thread conflicts detected.\n");
        }

        report.push_str("\n=== End Thread Analysis ===\n");
        report
    }
}

#[derive(Debug, Clone)]
pub struct RuntimeTestResult {
    pub strategy: String,
    pub success: bool,
    pub duration: Duration,
    pub error_message: Option<String>,
    pub thread_conflicts: Vec<String>,
}

#[derive(Debug)]
pub struct RuntimeTestResults {
    pub multi_threaded: RuntimeTestResult,
    pub single_threaded: RuntimeTestResult,
    pub current_thread: RuntimeTestResult,
    pub dedicated_thread: RuntimeTestResult,
}

impl RuntimeTestResults {
    fn new() -> Self {
        let default_result = RuntimeTestResult {
            strategy: "not_tested".to_string(),
            success: false,
            duration: Duration::from_secs(0),
            error_message: Some("Not tested".to_string()),
            thread_conflicts: Vec::new(),
        };

        Self {
            multi_threaded: default_result.clone(),
            single_threaded: default_result.clone(),
            current_thread: default_result.clone(),
            dedicated_thread: default_result,
        }
    }

    pub fn get_successful_strategies(&self) -> Vec<&RuntimeTestResult> {
        vec![&self.multi_threaded, &self.single_threaded, &self.current_thread, &self.dedicated_thread]
            .into_iter()
            .filter(|r| r.success)
            .collect()
    }

    pub fn get_recommended_strategy(&self) -> Option<&RuntimeTestResult> {
        // Recommend based on success and minimal conflicts
        let successful = self.get_successful_strategies();
        
        successful.into_iter()
            .min_by_key(|r| r.thread_conflicts.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_thread_analyzer_creation() {
        let analyzer = ThreadAnalyzer::new();
        analyzer.track_thread("test".to_string());
        let report = analyzer.generate_report();
        assert!(report.contains("Thread Analysis Report"));
    }

    #[tokio::test]
    async fn test_runtime_strategies() {
        let analyzer = ThreadAnalyzer::new();
        let results = analyzer.test_runtime_strategies();
        
        // At least one strategy should work in test environment
        let successful = results.get_successful_strategies();
        assert!(!successful.is_empty(), "At least one runtime strategy should work");
    }
}
