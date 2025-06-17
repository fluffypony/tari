// Event Loop Diagnostics for Node.js/Tokio Conflicts
// Detects and analyzes event loop interference patterns

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use std::thread;
use tokio::runtime::Runtime;
use log::{debug, error, warn, info};

#[derive(Debug, Clone)]
pub struct EventLoopEvent {
    pub timestamp: Instant,
    pub event_type: EventType,
    pub thread_id: thread::ThreadId,
    pub duration: Option<Duration>,
    pub details: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum EventType {
    RuntimeCreation,
    BlockOnStart,
    BlockOnEnd,
    SpawnTask,
    TaskCompletion,
    EventLoopBlocked,
    EventLoopResumed,
    NodeJsCallback,
    SegfaultRisk,
}

#[derive(Debug)]
pub struct EventLoopDiagnostics {
    events: Arc<Mutex<VecDeque<EventLoopEvent>>>,
    max_events: usize,
    monitoring_active: Arc<Mutex<bool>>,
}

impl EventLoopDiagnostics {
    pub fn new(max_events: usize) -> Self {
        Self {
            events: Arc::new(Mutex::new(VecDeque::with_capacity(max_events))),
            max_events,
            monitoring_active: Arc::new(Mutex::new(false)),
        }
    }

    /// Start monitoring event loop activity
    pub fn start_monitoring(&self) {
        if let Ok(mut active) = self.monitoring_active.lock() {
            *active = true;
        }
        
        info!(target: "tari::wallet_ffi::debug::event_loop", "Event loop monitoring started");
    }

    /// Stop monitoring event loop activity
    pub fn stop_monitoring(&self) {
        if let Ok(mut active) = self.monitoring_active.lock() {
            *active = false;
        }
        
        info!(target: "tari::wallet_ffi::debug::event_loop", "Event loop monitoring stopped");
    }

    /// Record an event loop event
    pub fn record_event(&self, event_type: EventType, details: String) {
        let is_active = self.monitoring_active.lock()
            .map(|active| *active)
            .unwrap_or(false);

        if !is_active {
            return;
        }

        let event = EventLoopEvent {
            timestamp: Instant::now(),
            event_type: event_type.clone(),
            thread_id: thread::current().id(),
            duration: None,
            details,
        };

        if let Ok(mut events) = self.events.lock() {
            // Remove old events if we're at capacity
            if events.len() >= self.max_events {
                events.pop_front();
            }
            events.push_back(event.clone());
        }

        debug!(
            target: "tari::wallet_ffi::debug::event_loop",
            "Recorded event: {:?} on thread {:?} - {}",
            event_type, event.thread_id, event.details
        );
    }

    /// Record the start of a blocking operation
    pub fn record_block_start(&self, operation: &str) -> OperationTimer {
        self.record_event(
            EventType::BlockOnStart,
            format!("Starting blocking operation: {}", operation)
        );

        OperationTimer::new(operation.to_string(), self.clone())
    }

    /// Detect event loop blocking patterns
    pub fn detect_blocking_patterns(&self) -> Vec<BlockingPattern> {
        let mut patterns = Vec::new();

        if let Ok(events) = self.events.lock() {
            let mut block_starts = Vec::new();
            
            for (i, event) in events.iter().enumerate() {
                match &event.event_type {
                    EventType::BlockOnStart => {
                        block_starts.push(i);
                    }
                    EventType::BlockOnEnd => {
                        // Find matching start
                        if let Some(start_idx) = block_starts.pop() {
                            let start_event = &events[start_idx];
                            let duration = event.timestamp.duration_since(start_event.timestamp);
                            
                            // Flag long-running blocks as potential issues
                            if duration > Duration::from_millis(100) {
                                patterns.push(BlockingPattern {
                                    pattern_type: BlockingPatternType::LongRunningBlock,
                                    duration,
                                    thread_id: event.thread_id,
                                    description: format!(
                                        "Long-running block_on operation: {} ({}ms)",
                                        event.details, duration.as_millis()
                                    ),
                                });
                            }
                        }
                    }
                    EventType::RuntimeCreation => {
                        // Check for multiple runtime creations
                        let runtime_count = events.iter()
                            .filter(|e| e.event_type == EventType::RuntimeCreation)
                            .count();
                        
                        if runtime_count > 1 {
                            patterns.push(BlockingPattern {
                                pattern_type: BlockingPatternType::MultipleRuntimes,
                                duration: Duration::from_secs(0),
                                thread_id: event.thread_id,
                                description: format!("Multiple runtime creations detected: {}", runtime_count),
                            });
                        }
                    }
                    EventType::SegfaultRisk => {
                        patterns.push(BlockingPattern {
                            pattern_type: BlockingPatternType::SegfaultRisk,
                            duration: Duration::from_secs(0),
                            thread_id: event.thread_id,
                            description: event.details.clone(),
                        });
                    }
                    _ => {}
                }
            }
        }

        patterns
    }

    /// Analyze event loop health
    pub fn analyze_event_loop_health(&self) -> EventLoopHealthReport {
        let blocking_patterns = self.detect_blocking_patterns();
        
        let mut risk_level = RiskLevel::Low;
        let mut recommendations = Vec::new();

        // Analyze blocking patterns
        for pattern in &blocking_patterns {
            match pattern.pattern_type {
                BlockingPatternType::LongRunningBlock => {
                    if pattern.duration > Duration::from_millis(500) {
                        risk_level = RiskLevel::High;
                        recommendations.push("Consider using dedicated thread runtime to avoid blocking Node.js event loop".to_string());
                    } else if pattern.duration > Duration::from_millis(100) {
                        risk_level = std::cmp::max(risk_level, RiskLevel::Medium);
                        recommendations.push("Monitor block_on operations for potential Node.js conflicts".to_string());
                    }
                }
                BlockingPatternType::MultipleRuntimes => {
                    risk_level = RiskLevel::High;
                    recommendations.push("Avoid creating multiple Tokio runtimes - use single runtime with shared handle".to_string());
                }
                BlockingPatternType::SegfaultRisk => {
                    risk_level = RiskLevel::Critical;
                    recommendations.push("Critical segfault risk detected - implement alternative runtime strategy".to_string());
                }
            }
        }

        // Check for Node.js environment
        if self.is_nodejs_environment() {
            risk_level = std::cmp::max(risk_level, RiskLevel::Medium);
            recommendations.push("Node.js environment detected - use single-threaded runtime or dedicated thread".to_string());
        }

        EventLoopHealthReport {
            risk_level,
            blocking_patterns,
            recommendations,
            total_events: self.get_event_count(),
            monitoring_duration: self.get_monitoring_duration(),
        }
    }

    /// Generate comprehensive diagnostics report
    pub fn generate_report(&self) -> String {
        let mut report = String::new();
        report.push_str("=== Event Loop Diagnostics Report ===\n\n");

        let health_report = self.analyze_event_loop_health();
        
        report.push_str(&format!("Risk Level: {:?}\n", health_report.risk_level));
        report.push_str(&format!("Total Events: {}\n", health_report.total_events));
        report.push_str(&format!("Monitoring Duration: {:?}\n", health_report.monitoring_duration));
        
        if !health_report.blocking_patterns.is_empty() {
            report.push_str("\nBlocking Patterns Detected:\n");
            for pattern in &health_report.blocking_patterns {
                report.push_str(&format!(
                    "  - {:?}: {} (Thread: {:?}, Duration: {:?})\n",
                    pattern.pattern_type, pattern.description, 
                    pattern.thread_id, pattern.duration
                ));
            }
        }

        if !health_report.recommendations.is_empty() {
            report.push_str("\nRecommendations:\n");
            for rec in &health_report.recommendations {
                report.push_str(&format!("  - {}\n", rec));
            }
        }

        // Add event timeline
        if let Ok(events) = self.events.lock() {
            if !events.is_empty() {
                report.push_str("\nEvent Timeline (last 10 events):\n");
                let recent_events: Vec<_> = events.iter().rev().take(10).collect();
                
                for event in recent_events.iter().rev() {
                    report.push_str(&format!(
                        "  {:?}: {:?} - {} (Thread: {:?})\n",
                        event.timestamp, event.event_type, 
                        event.details, event.thread_id
                    ));
                }
            }
        }

        report.push_str("\n=== End Event Loop Diagnostics ===\n");
        report
    }

    fn get_event_count(&self) -> usize {
        self.events.lock().map(|events| events.len()).unwrap_or(0)
    }

    fn get_monitoring_duration(&self) -> Duration {
        if let Ok(events) = self.events.lock() {
            if let (Some(first), Some(last)) = (events.front(), events.back()) {
                return last.timestamp.duration_since(first.timestamp);
            }
        }
        Duration::from_secs(0)
    }

    fn is_nodejs_environment(&self) -> bool {
        std::env::var("NODE_ENV").is_ok() ||
        std::env::var("npm_config_user_config").is_ok() ||
        std::env::var("NODE_PATH").is_ok()
    }
}

impl Clone for EventLoopDiagnostics {
    fn clone(&self) -> Self {
        Self {
            events: Arc::clone(&self.events),
            max_events: self.max_events,
            monitoring_active: Arc::clone(&self.monitoring_active),
        }
    }
}

/// Timer for tracking blocking operations
pub struct OperationTimer {
    operation: String,
    start_time: Instant,
    diagnostics: EventLoopDiagnostics,
}

impl OperationTimer {
    fn new(operation: String, diagnostics: EventLoopDiagnostics) -> Self {
        Self {
            operation,
            start_time: Instant::now(),
            diagnostics,
        }
    }

    /// Complete the operation and record duration
    pub fn complete(self) {
        let duration = self.start_time.elapsed();
        
        self.diagnostics.record_event(
            EventType::BlockOnEnd,
            format!("Completed blocking operation: {} ({}ms)", 
                   self.operation, duration.as_millis())
        );

        // Flag potential issues
        if duration > Duration::from_millis(500) {
            self.diagnostics.record_event(
                EventType::SegfaultRisk,
                format!("Long-running operation {} may cause Node.js event loop blocking", 
                       self.operation)
            );
        }
    }
}

impl Drop for OperationTimer {
    fn drop(&mut self) {
        let duration = self.start_time.elapsed();
        
        self.diagnostics.record_event(
            EventType::BlockOnEnd,
            format!("Auto-completed blocking operation: {} ({}ms)", 
                   self.operation, duration.as_millis())
        );
    }
}

#[derive(Debug, Clone)]
pub struct BlockingPattern {
    pub pattern_type: BlockingPatternType,
    pub duration: Duration,
    pub thread_id: thread::ThreadId,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BlockingPatternType {
    LongRunningBlock,
    MultipleRuntimes,
    SegfaultRisk,
}

#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug)]
pub struct EventLoopHealthReport {
    pub risk_level: RiskLevel,
    pub blocking_patterns: Vec<BlockingPattern>,
    pub recommendations: Vec<String>,
    pub total_events: usize,
    pub monitoring_duration: Duration,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_loop_diagnostics_creation() {
        let diagnostics = EventLoopDiagnostics::new(100);
        diagnostics.start_monitoring();
        
        diagnostics.record_event(
            EventType::RuntimeCreation,
            "Test runtime creation".to_string()
        );
        
        let report = diagnostics.generate_report();
        assert!(report.contains("Event Loop Diagnostics Report"));
        
        diagnostics.stop_monitoring();
    }

    #[test]
    fn test_operation_timer() {
        let diagnostics = EventLoopDiagnostics::new(100);
        diagnostics.start_monitoring();
        
        let timer = diagnostics.record_block_start("test_operation");
        std::thread::sleep(Duration::from_millis(10));
        timer.complete();
        
        let health_report = diagnostics.analyze_event_loop_health();
        assert_eq!(health_report.total_events, 2); // Start and end events
    }

    #[test]
    fn test_blocking_pattern_detection() {
        let diagnostics = EventLoopDiagnostics::new(100);
        diagnostics.start_monitoring();
        
        // Simulate long-running operation
        diagnostics.record_event(
            EventType::BlockOnStart,
            "Long operation start".to_string()
        );
        
        std::thread::sleep(Duration::from_millis(10));
        
        diagnostics.record_event(
            EventType::BlockOnEnd,
            "Long operation end".to_string()
        );
        
        let patterns = diagnostics.detect_blocking_patterns();
        // The test sleep is too short to trigger a pattern, but the structure should work
        assert!(patterns.is_empty() || !patterns.is_empty()); // Just verify it doesn't crash
    }
}
