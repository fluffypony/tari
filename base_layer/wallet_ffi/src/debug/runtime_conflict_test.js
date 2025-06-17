#!/usr/bin/env node
/**
 * Node.js Runtime Conflict Test
 * 
 * Tests for detecting and analyzing Tokio runtime conflicts with Node.js event loop.
 * This script simulates the conditions that cause segfaults in the Tari wallet FFI.
 */

const fs = require('fs');
const path = require('path');
const { performance } = require('perf_hooks');

class RuntimeConflictTester {
    constructor() {
        this.results = [];
        this.conflictDetected = false;
        this.testStartTime = performance.now();
    }

    /**
     * Log test information
     */
    log(message) {
        const timestamp = new Date().toISOString();
        const elapsed = (performance.now() - this.testStartTime).toFixed(2);
        console.log(`[${timestamp}] [+${elapsed}ms] ${message}`);
    }

    /**
     * Log error information
     */
    error(message) {
        const timestamp = new Date().toISOString();
        const elapsed = (performance.now() - this.testStartTime).toFixed(2);
        console.error(`[${timestamp}] [+${elapsed}ms] ERROR: ${message}`);
    }

    /**
     * Detect Node.js environment information
     */
    detectEnvironment() {
        this.log("Detecting Node.js environment...");
        
        const env = {
            nodeVersion: process.version,
            platform: process.platform,
            arch: process.arch,
            pid: process.pid,
            ppid: process.ppid,
            execPath: process.execPath,
            title: process.title,
            argv0: process.argv0,
            mainModule: require.main ? require.main.filename : 'unknown',
            eventLoopUtilization: process.cpuUsage(),
            memoryUsage: process.memoryUsage(),
            resourceUsage: process.resourceUsage ? process.resourceUsage() : null,
            features: {
                inspector: typeof process.inspector !== 'undefined',
                worker_threads: (() => {
                    try {
                        require('worker_threads');
                        return true;
                    } catch {
                        return false;
                    }
                })(),
                async_hooks: (() => {
                    try {
                        require('async_hooks');
                        return true;
                    } catch {
                        return false;
                    }
                })(),
            }
        };

        this.log(`Node.js version: ${env.nodeVersion}`);
        this.log(`Platform: ${env.platform}/${env.arch}`);
        this.log(`Process ID: ${env.pid}`);
        this.log(`Memory usage: ${JSON.stringify(env.memoryUsage)}`);
        
        return env;
    }

    /**
     * Test event loop characteristics
     */
    async testEventLoop() {
        this.log("Testing event loop characteristics...");
        
        return new Promise((resolve) => {
            const results = {
                timers: [],
                immediates: [],
                nextTicks: [],
                promises: [],
                totalTime: 0
            };

            const startTime = performance.now();

            // Test setImmediate precision
            for (let i = 0; i < 10; i++) {
                const immediateStart = performance.now();
                setImmediate(() => {
                    const elapsed = performance.now() - immediateStart;
                    results.immediates.push(elapsed);
                });
            }

            // Test setTimeout precision
            for (let i = 0; i < 10; i++) {
                const timerStart = performance.now();
                setTimeout(() => {
                    const elapsed = performance.now() - timerStart;
                    results.timers.push(elapsed);
                }, 0);
            }

            // Test process.nextTick precision
            for (let i = 0; i < 10; i++) {
                const nextTickStart = performance.now();
                process.nextTick(() => {
                    const elapsed = performance.now() - nextTickStart;
                    results.nextTicks.push(elapsed);
                });
            }

            // Test Promise resolution precision
            for (let i = 0; i < 10; i++) {
                const promiseStart = performance.now();
                Promise.resolve().then(() => {
                    const elapsed = performance.now() - promiseStart;
                    results.promises.push(elapsed);
                });
            }

            // Wait for all callbacks to complete
            setTimeout(() => {
                results.totalTime = performance.now() - startTime;
                
                this.log(`Event loop test completed in ${results.totalTime.toFixed(2)}ms`);
                this.log(`Average setImmediate delay: ${this.average(results.immediates).toFixed(2)}ms`);
                this.log(`Average setTimeout delay: ${this.average(results.timers).toFixed(2)}ms`);
                this.log(`Average nextTick delay: ${this.average(results.nextTicks).toFixed(2)}ms`);
                this.log(`Average Promise delay: ${this.average(results.promises).toFixed(2)}ms`);
                
                resolve(results);
            }, 100);
        });
    }

    /**
     * Test high-frequency callback scenarios
     */
    async testHighFrequencyCallbacks() {
        this.log("Testing high-frequency callback scenarios...");
        
        return new Promise((resolve) => {
            const results = {
                callbackCount: 0,
                errors: [],
                maxLatency: 0,
                avgLatency: 0,
                totalTime: 0
            };

            const startTime = performance.now();
            const targetCallbacks = 10000;
            const latencies = [];

            const scheduleCallback = (index) => {
                const callbackStart = performance.now();
                
                setImmediate(() => {
                    try {
                        const latency = performance.now() - callbackStart;
                        latencies.push(latency);
                        results.callbackCount++;
                        
                        if (latency > results.maxLatency) {
                            results.maxLatency = latency;
                        }

                        // Simulate some work (like FFI call would do)
                        const workStart = performance.now();
                        let sum = 0;
                        for (let i = 0; i < 1000; i++) {
                            sum += Math.sqrt(i);
                        }
                        const workTime = performance.now() - workStart;

                        if (workTime > 10) { // Suspicious work time
                            this.conflictDetected = true;
                            results.errors.push(`Excessive work time at callback ${index}: ${workTime.toFixed(2)}ms`);
                        }

                        if (index < targetCallbacks - 1) {
                            scheduleCallback(index + 1);
                        } else {
                            // Test complete
                            results.totalTime = performance.now() - startTime;
                            results.avgLatency = this.average(latencies);
                            
                            this.log(`High-frequency test completed: ${results.callbackCount} callbacks in ${results.totalTime.toFixed(2)}ms`);
                            this.log(`Average latency: ${results.avgLatency.toFixed(2)}ms, Max latency: ${results.maxLatency.toFixed(2)}ms`);
                            
                            if (results.errors.length > 0) {
                                this.error(`${results.errors.length} timing anomalies detected`);
                                results.errors.slice(0, 5).forEach(err => this.error(err));
                            }
                            
                            resolve(results);
                        }
                    } catch (error) {
                        results.errors.push(`Callback ${index} failed: ${error.message}`);
                        this.error(`Callback ${index} failed: ${error.message}`);
                        resolve(results);
                    }
                });
            };

            scheduleCallback(0);
        });
    }

    /**
     * Test memory pressure scenarios
     */
    async testMemoryPressure() {
        this.log("Testing memory pressure scenarios...");
        
        const results = {
            allocations: 0,
            deallocations: 0,
            peakMemory: 0,
            errors: [],
            gcEvents: 0
        };

        // Monitor GC events
        if (global.gc) {
            const originalGc = global.gc;
            global.gc = function() {
                results.gcEvents++;
                return originalGc.apply(this, arguments);
            };
        }

        const startMemory = process.memoryUsage();
        const allocatedBuffers = [];

        try {
            // Allocate memory in patterns similar to FFI usage
            for (let i = 0; i < 1000; i++) {
                // Simulate various buffer sizes that FFI might use
                const sizes = [64, 256, 1024, 4096, 16384];
                const size = sizes[i % sizes.length];
                
                const buffer = Buffer.alloc(size);
                buffer.fill(i % 256);
                allocatedBuffers.push(buffer);
                results.allocations++;

                // Periodically check memory usage
                if (i % 100 === 0) {
                    const currentMemory = process.memoryUsage();
                    if (currentMemory.heapUsed > results.peakMemory) {
                        results.peakMemory = currentMemory.heapUsed;
                    }

                    // Simulate some buffers being freed (as FFI would do)
                    if (allocatedBuffers.length > 500) {
                        for (let j = 0; j < 100; j++) {
                            allocatedBuffers.shift();
                            results.deallocations++;
                        }
                    }
                }
            }

            // Force garbage collection if available
            if (global.gc) {
                global.gc();
            }

            const endMemory = process.memoryUsage();
            
            this.log(`Memory pressure test completed:`);
            this.log(`  Allocations: ${results.allocations}, Deallocations: ${results.deallocations}`);
            this.log(`  Start heap: ${Math.round(startMemory.heapUsed / 1024 / 1024)}MB`);
            this.log(`  Peak heap: ${Math.round(results.peakMemory / 1024 / 1024)}MB`);
            this.log(`  End heap: ${Math.round(endMemory.heapUsed / 1024 / 1024)}MB`);
            this.log(`  GC events: ${results.gcEvents}`);

        } catch (error) {
            results.errors.push(`Memory pressure test failed: ${error.message}`);
            this.error(`Memory pressure test failed: ${error.message}`);
        }

        return results;
    }

    /**
     * Test Worker Thread interactions
     */
    async testWorkerThreads() {
        this.log("Testing Worker Thread interactions...");
        
        try {
            const { Worker, isMainThread, parentPort } = require('worker_threads');
            
            if (!isMainThread) {
                this.log("Running in worker thread - sending test message");
                parentPort.postMessage({ type: 'test', data: 'worker_test_data' });
                return { workerSupported: true, isWorker: true };
            }

            return new Promise((resolve) => {
                const results = {
                    workerSupported: true,
                    isWorker: false,
                    workerMessages: [],
                    errors: []
                };

                // Create a simple worker for testing
                const workerCode = `
                    const { parentPort } = require('worker_threads');
                    
                    // Simulate some work that might conflict with Tokio
                    setInterval(() => {
                        parentPort.postMessage({
                            type: 'heartbeat',
                            timestamp: Date.now(),
                            memoryUsage: process.memoryUsage()
                        });
                    }, 100);
                    
                    setTimeout(() => {
                        parentPort.postMessage({ type: 'complete' });
                    }, 1000);
                `;

                const worker = new Worker(workerCode, { eval: true });
                
                worker.on('message', (message) => {
                    results.workerMessages.push(message);
                    
                    if (message.type === 'complete') {
                        worker.terminate();
                        
                        this.log(`Worker thread test completed: ${results.workerMessages.length} messages received`);
                        resolve(results);
                    }
                });

                worker.on('error', (error) => {
                    results.errors.push(`Worker error: ${error.message}`);
                    this.error(`Worker error: ${error.message}`);
                    resolve(results);
                });

                // Timeout fallback
                setTimeout(() => {
                    worker.terminate();
                    results.errors.push('Worker thread test timeout');
                    this.error('Worker thread test timeout');
                    resolve(results);
                }, 5000);
            });

        } catch (error) {
            this.log(`Worker threads not supported: ${error.message}`);
            return { workerSupported: false, error: error.message };
        }
    }

    /**
     * Simulate FFI call patterns
     */
    async testFFICallPatterns() {
        this.log("Testing FFI call patterns simulation...");
        
        const results = {
            syncCalls: 0,
            asyncCalls: 0,
            callbackCalls: 0,
            errors: [],
            timings: []
        };

        // Simulate synchronous FFI calls (blocking)
        for (let i = 0; i < 100; i++) {
            const start = performance.now();
            
            // Simulate blocking work (like Rust FFI might do)
            const blockingWork = () => {
                let sum = 0;
                for (let j = 0; j < 100000; j++) {
                    sum += Math.sqrt(j);
                }
                return sum;
            };

            const result = blockingWork();
            const elapsed = performance.now() - start;
            
            results.syncCalls++;
            results.timings.push({ type: 'sync', elapsed });

            if (elapsed > 10) { // Suspicious timing
                this.conflictDetected = true;
                results.errors.push(`Slow sync call ${i}: ${elapsed.toFixed(2)}ms`);
            }
        }

        // Simulate asynchronous FFI calls
        const asyncPromises = [];
        for (let i = 0; i < 100; i++) {
            const promise = new Promise((resolve) => {
                const start = performance.now();
                
                setImmediate(() => {
                    // Simulate async work
                    const work = Math.sqrt(Math.random() * 1000000);
                    const elapsed = performance.now() - start;
                    
                    results.asyncCalls++;
                    results.timings.push({ type: 'async', elapsed });
                    
                    resolve(work);
                });
            });
            asyncPromises.push(promise);
        }

        await Promise.all(asyncPromises);

        // Simulate callback-based FFI calls
        await new Promise((resolve) => {
            let completed = 0;
            const total = 100;

            for (let i = 0; i < total; i++) {
                const start = performance.now();
                
                // Simulate callback after some delay
                setTimeout(() => {
                    const elapsed = performance.now() - start;
                    results.callbackCalls++;
                    results.timings.push({ type: 'callback', elapsed });
                    
                    completed++;
                    if (completed === total) {
                        resolve();
                    }
                }, Math.random() * 10);
            }
        });

        const avgSync = this.average(results.timings.filter(t => t.type === 'sync').map(t => t.elapsed));
        const avgAsync = this.average(results.timings.filter(t => t.type === 'async').map(t => t.elapsed));
        const avgCallback = this.average(results.timings.filter(t => t.type === 'callback').map(t => t.elapsed));

        this.log(`FFI pattern test completed:`);
        this.log(`  Sync calls: ${results.syncCalls} (avg: ${avgSync.toFixed(2)}ms)`);
        this.log(`  Async calls: ${results.asyncCalls} (avg: ${avgAsync.toFixed(2)}ms)`);
        this.log(`  Callback calls: ${results.callbackCalls} (avg: ${avgCallback.toFixed(2)}ms)`);

        if (results.errors.length > 0) {
            this.error(`${results.errors.length} timing anomalies in FFI simulation`);
        }

        return results;
    }

    /**
     * Calculate average of array
     */
    average(arr) {
        return arr.length > 0 ? arr.reduce((a, b) => a + b, 0) / arr.length : 0;
    }

    /**
     * Run all conflict tests
     */
    async runAllTests() {
        this.log("Starting comprehensive Node.js runtime conflict tests...");
        
        const environment = this.detectEnvironment();
        const eventLoopResults = await this.testEventLoop();
        const highFreqResults = await this.testHighFrequencyCallbacks();
        const memoryResults = await this.testMemoryPressure();
        const workerResults = await this.testWorkerThreads();
        const ffiResults = await this.testFFICallPatterns();

        const totalTime = performance.now() - this.testStartTime;

        const summary = {
            environment,
            testResults: {
                eventLoop: eventLoopResults,
                highFrequency: highFreqResults,
                memory: memoryResults,
                workers: workerResults,
                ffi: ffiResults
            },
            conflictDetected: this.conflictDetected,
            totalTestTime: totalTime
        };

        this.generateReport(summary);
        return summary;
    }

    /**
     * Generate comprehensive test report
     */
    generateReport(summary) {
        this.log("Generating test report...");

        const report = {
            timestamp: new Date().toISOString(),
            summary: {
                conflictDetected: summary.conflictDetected,
                totalTestTime: summary.totalTestTime,
                nodeVersion: summary.environment.nodeVersion,
                platform: `${summary.environment.platform}/${summary.environment.arch}`
            },
            details: summary,
            recommendations: this.generateRecommendations(summary)
        };

        // Write report to file
        const reportPath = path.join(__dirname, 'runtime_conflict_report.json');
        fs.writeFileSync(reportPath, JSON.stringify(report, null, 2));

        this.log(`Report written to: ${reportPath}`);
        
        // Print summary
        console.log("\n=== RUNTIME CONFLICT TEST SUMMARY ===");
        console.log(`Conflict Detected: ${summary.conflictDetected ? 'YES' : 'NO'}`);
        console.log(`Total Test Time: ${summary.totalTestTime.toFixed(2)}ms`);
        console.log(`Node.js Version: ${summary.environment.nodeVersion}`);
        console.log(`Platform: ${summary.environment.platform}/${summary.environment.arch}`);
        
        if (report.recommendations.length > 0) {
            console.log("\nRecommendations:");
            report.recommendations.forEach((rec, i) => {
                console.log(`${i + 1}. ${rec}`);
            });
        }
    }

    /**
     * Generate recommendations based on test results
     */
    generateRecommendations(summary) {
        const recommendations = [];

        if (this.conflictDetected) {
            recommendations.push("Runtime conflicts detected - consider using dedicated thread strategy for Tokio");
        }

        if (summary.testResults.memory.peakMemory > 100 * 1024 * 1024) { // 100MB
            recommendations.push("High memory usage detected - implement memory monitoring in FFI");
        }

        if (summary.testResults.highFrequency.maxLatency > 50) {
            recommendations.push("High callback latency detected - optimize FFI callback handling");
        }

        if (summary.testResults.workers.workerSupported && summary.testResults.workers.errors.length > 0) {
            recommendations.push("Worker thread issues detected - test FFI compatibility with worker threads");
        }

        if (!summary.testResults.workers.workerSupported) {
            recommendations.push("Worker threads not available - single-threaded runtime recommended");
        }

        return recommendations;
    }
}

// Run tests if called directly
if (require.main === module) {
    const tester = new RuntimeConflictTester();
    tester.runAllTests().then(() => {
        process.exit(0);
    }).catch((error) => {
        console.error("Test execution failed:", error);
        process.exit(1);
    });
}

module.exports = RuntimeConflictTester;
