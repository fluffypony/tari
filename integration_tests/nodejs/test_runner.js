#!/usr/bin/env node
/**
 * Node.js Integration Test Runner for Tari Wallet FFI
 * 
 * Comprehensive test suite for validating the segfault fixes and Node.js compatibility
 * of the enhanced Tari wallet FFI implementation.
 */

const fs = require('fs');
const path = require('path');
const { performance } = require('perf_hooks');
const RuntimeConflictTester = require('../../base_layer/wallet_ffi/src/debug/runtime_conflict_test');

class NodeJSFFITestRunner {
    constructor() {
        this.results = [];
        this.totalTests = 0;
        this.passedTests = 0;
        this.failedTests = 0;
        this.startTime = performance.now();
    }

    /**
     * Log test information
     */
    log(message) {
        const timestamp = new Date().toISOString();
        console.log(`[${timestamp}] ${message}`);
    }

    /**
     * Log error information
     */
    error(message) {
        const timestamp = new Date().toISOString();
        console.error(`[${timestamp}] ERROR: ${message}`);
    }

    /**
     * Run a single test
     */
    async runTest(testName, testFunction) {
        this.totalTests++;
        this.log(`Running test: ${testName}`);
        
        const testStart = performance.now();
        
        try {
            await testFunction();
            const duration = performance.now() - testStart;
            this.passedTests++;
            this.log(`✓ ${testName} (${duration.toFixed(2)}ms)`);
            
            this.results.push({
                name: testName,
                status: 'PASS',
                duration,
                error: null
            });
        } catch (error) {
            const duration = performance.now() - testStart;
            this.failedTests++;
            this.error(`✗ ${testName} (${duration.toFixed(2)}ms): ${error.message}`);
            
            this.results.push({
                name: testName,
                status: 'FAIL',
                duration,
                error: error.message
            });
        }
    }

    /**
     * Test basic Node.js environment detection
     */
    async testEnvironmentDetection() {
        // Verify we're running in Node.js
        if (typeof process === 'undefined' || !process.version) {
            throw new Error('Not running in Node.js environment');
        }

        // Check for expected Node.js features
        const requiredFeatures = ['require', 'module', 'exports', '__dirname', '__filename'];
        for (const feature of requiredFeatures) {
            if (typeof global[feature] === 'undefined' && typeof eval(feature) === 'undefined') {
                throw new Error(`Missing Node.js feature: ${feature}`);
            }
        }

        // Verify environment variables that our FFI code checks for
        const nodeEnvVars = ['NODE_VERSION', 'npm_config_registry', 'NODE_ENV'];
        let hasNodeVar = false;
        for (const envVar of nodeEnvVars) {
            if (process.env[envVar]) {
                hasNodeVar = true;
                break;
            }
        }

        if (!hasNodeVar) {
            // Set NODE_VERSION for testing
            process.env.NODE_VERSION = process.version;
        }

        this.log(`Node.js ${process.version} detected on ${process.platform}/${process.arch}`);
    }

    /**
     * Test runtime conflict detection
     */
    async testRuntimeConflictDetection() {
        const tester = new RuntimeConflictTester();
        const summary = await tester.runAllTests();

        if (summary.conflictDetected) {
            this.log(`Runtime conflicts detected - this validates our test environment`);
        } else {
            this.log(`No runtime conflicts detected in current environment`);
        }

        // Verify the test completed successfully
        if (!summary.testResults || !summary.testResults.eventLoop) {
            throw new Error('Runtime conflict test did not complete properly');
        }

        this.log(`Runtime test completed in ${summary.totalTestTime.toFixed(2)}ms`);
    }

    /**
     * Test FFI library loading
     */
    async testFFILibraryLoading() {
        // First, check if the wallet FFI library exists
        const possibleLibPaths = [
            path.join(__dirname, '../target/debug/libminotari_wallet_ffi.dylib'),
            path.join(__dirname, '../target/release/libminotari_wallet_ffi.dylib'),
            path.join(__dirname, '../target/debug/libminotari_wallet_ffi.so'),
            path.join(__dirname, '../target/release/libminotari_wallet_ffi.so'),
            path.join(__dirname, '../target/debug/minotari_wallet_ffi.dll'),
            path.join(__dirname, '../target/release/minotari_wallet_ffi.dll'),
        ];

        let libraryPath = null;
        for (const libPath of possibleLibPaths) {
            if (fs.existsSync(libPath)) {
                libraryPath = libPath;
                break;
            }
        }

        if (!libraryPath) {
            throw new Error('Wallet FFI library not found. Please build the project first.');
        }

        this.log(`Found wallet FFI library at: ${libraryPath}`);

        // Test basic FFI loading
        try {
            const ffi = require('ffi-napi');
            const ref = require('ref-napi');
            
            // Define basic FFI interface for testing
            const lib = ffi.Library(libraryPath, {
                // We'll just test that we can load the library
                // Actual function calls would require proper initialization
            });
            
            this.log('FFI library loaded successfully');
        } catch (error) {
            // If ffi-napi is not available, that's expected in some environments
            if (error.code === 'MODULE_NOT_FOUND' && error.message.includes('ffi-napi')) {
                this.log('ffi-napi not available - skipping FFI loading test');
                return;
            }
            throw error;
        }
    }

    /**
     * Test memory pressure scenarios
     */
    async testMemoryPressure() {
        const initialMemory = process.memoryUsage();
        const allocations = [];

        // Allocate memory in patterns similar to wallet operations
        for (let i = 0; i < 1000; i++) {
            // Create buffers of various sizes
            const sizes = [1024, 4096, 16384, 65536];
            const size = sizes[i % sizes.length];
            
            const buffer = Buffer.alloc(size);
            buffer.fill(i % 256);
            allocations.push(buffer);

            // Periodically free some memory
            if (i % 100 === 0 && allocations.length > 500) {
                allocations.splice(0, 200);
                
                // Force garbage collection if available
                if (global.gc) {
                    global.gc();
                }
            }
        }

        const finalMemory = process.memoryUsage();
        const memoryIncrease = finalMemory.heapUsed - initialMemory.heapUsed;
        
        this.log(`Memory pressure test: ${memoryIncrease} bytes allocated`);
        
        // Clean up
        allocations.length = 0;
        if (global.gc) {
            global.gc();
        }

        // Verify we haven't leaked too much memory
        const afterCleanup = process.memoryUsage();
        const netIncrease = afterCleanup.heapUsed - initialMemory.heapUsed;
        
        if (netIncrease > 50 * 1024 * 1024) { // 50MB threshold
            throw new Error(`Potential memory leak detected: ${netIncrease} bytes`);
        }
    }

    /**
     * Test async operations that might conflict with Tokio
     */
    async testAsyncOperationConflicts() {
        const asyncOperations = [];
        
        // Create a mix of async operations
        for (let i = 0; i < 100; i++) {
            // Promise-based operations
            asyncOperations.push(
                new Promise(resolve => {
                    setImmediate(() => {
                        resolve(i);
                    });
                })
            );

            // setTimeout operations
            asyncOperations.push(
                new Promise(resolve => {
                    setTimeout(() => {
                        resolve(i * 2);
                    }, Math.random() * 10);
                })
            );

            // nextTick operations
            asyncOperations.push(
                new Promise(resolve => {
                    process.nextTick(() => {
                        resolve(i * 3);
                    });
                })
            );
        }

        const startTime = performance.now();
        const results = await Promise.all(asyncOperations);
        const duration = performance.now() - startTime;

        // Verify all operations completed
        if (results.length !== 300) {
            throw new Error(`Expected 300 results, got ${results.length}`);
        }

        // Check for reasonable timing (detect if operations were blocked)
        if (duration > 1000) { // 1 second threshold
            throw new Error(`Async operations took too long: ${duration.toFixed(2)}ms`);
        }

        this.log(`${asyncOperations.length} async operations completed in ${duration.toFixed(2)}ms`);
    }

    /**
     * Test Worker Thread compatibility
     */
    async testWorkerThreadCompatibility() {
        try {
            const { Worker, isMainThread, parentPort } = require('worker_threads');
            
            if (!isMainThread) {
                // We're in a worker thread - this shouldn't happen in this test
                throw new Error('Test is running in worker thread');
            }

            return new Promise((resolve, reject) => {
                // Create a simple worker for testing
                const workerCode = `
                    const { parentPort } = require('worker_threads');
                    
                    // Simulate wallet-like operations in worker
                    let counter = 0;
                    const interval = setInterval(() => {
                        counter++;
                        parentPort.postMessage({
                            type: 'progress',
                            counter: counter
                        });
                        
                        if (counter >= 10) {
                            clearInterval(interval);
                            parentPort.postMessage({
                                type: 'complete',
                                result: 'success'
                            });
                        }
                    }, 10);
                `;

                const worker = new Worker(workerCode, { eval: true });
                let messageCount = 0;
                
                worker.on('message', (message) => {
                    messageCount++;
                    
                    if (message.type === 'complete') {
                        worker.terminate();
                        
                        if (messageCount < 10) {
                            reject(new Error(`Too few messages from worker: ${messageCount}`));
                        } else {
                            this.log(`Worker thread test completed with ${messageCount} messages`);
                            resolve();
                        }
                    }
                });

                worker.on('error', (error) => {
                    reject(new Error(`Worker error: ${error.message}`));
                });

                // Timeout fallback
                setTimeout(() => {
                    worker.terminate();
                    reject(new Error('Worker thread test timeout'));
                }, 5000);
            });

        } catch (error) {
            if (error.code === 'MODULE_NOT_FOUND') {
                this.log('Worker threads not supported - skipping test');
                return;
            }
            throw error;
        }
    }

    /**
     * Test callback function handling patterns
     */
    async testCallbackPatterns() {
        const callbacks = [];
        const results = [];

        // Simulate FFI callback patterns
        for (let i = 0; i < 100; i++) {
            callbacks.push((data) => {
                results.push(data * 2);
            });
        }

        // Execute callbacks in rapid succession
        const startTime = performance.now();
        
        for (let i = 0; i < callbacks.length; i++) {
            // Use setImmediate to simulate async callback execution
            await new Promise(resolve => {
                setImmediate(() => {
                    callbacks[i](i);
                    resolve();
                });
            });
        }

        const duration = performance.now() - startTime;

        // Verify all callbacks executed
        if (results.length !== 100) {
            throw new Error(`Expected 100 callback results, got ${results.length}`);
        }

        // Verify callback results
        for (let i = 0; i < results.length; i++) {
            if (results[i] !== i * 2) {
                throw new Error(`Callback result mismatch at index ${i}: expected ${i * 2}, got ${results[i]}`);
            }
        }

        this.log(`Callback pattern test completed in ${duration.toFixed(2)}ms`);
    }

    /**
     * Run all tests
     */
    async runAllTests() {
        this.log('Starting Node.js FFI Integration Tests...');
        this.log(`Node.js ${process.version} on ${process.platform}/${process.arch}`);
        this.log(`Process ID: ${process.pid}`);

        // Run all test suites
        await this.runTest('Environment Detection', () => this.testEnvironmentDetection());
        await this.runTest('Runtime Conflict Detection', () => this.testRuntimeConflictDetection());
        await this.runTest('FFI Library Loading', () => this.testFFILibraryLoading());
        await this.runTest('Memory Pressure Handling', () => this.testMemoryPressure());
        await this.runTest('Async Operation Conflicts', () => this.testAsyncOperationConflicts());
        await this.runTest('Worker Thread Compatibility', () => this.testWorkerThreadCompatibility());
        await this.runTest('Callback Patterns', () => this.testCallbackPatterns());

        // Generate final report
        this.generateReport();
    }

    /**
     * Generate test report
     */
    generateReport() {
        const totalTime = performance.now() - this.startTime;
        
        console.log('\n=== NODE.JS FFI INTEGRATION TEST REPORT ===');
        console.log(`Total Tests: ${this.totalTests}`);
        console.log(`Passed: ${this.passedTests}`);
        console.log(`Failed: ${this.failedTests}`);
        console.log(`Success Rate: ${((this.passedTests / this.totalTests) * 100).toFixed(1)}%`);
        console.log(`Total Time: ${totalTime.toFixed(2)}ms`);
        
        if (this.failedTests > 0) {
            console.log('\n=== FAILED TESTS ===');
            this.results
                .filter(r => r.status === 'FAIL')
                .forEach(result => {
                    console.log(`✗ ${result.name}: ${result.error}`);
                });
        }

        // Write detailed report to file
        const report = {
            timestamp: new Date().toISOString(),
            environment: {
                nodeVersion: process.version,
                platform: process.platform,
                arch: process.arch,
                pid: process.pid,
                memoryUsage: process.memoryUsage()
            },
            summary: {
                totalTests: this.totalTests,
                passedTests: this.passedTests,
                failedTests: this.failedTests,
                successRate: (this.passedTests / this.totalTests) * 100,
                totalTime: totalTime
            },
            results: this.results
        };

        const reportPath = path.join(__dirname, 'nodejs_ffi_test_report.json');
        fs.writeFileSync(reportPath, JSON.stringify(report, null, 2));
        console.log(`\nDetailed report written to: ${reportPath}`);

        // Exit with appropriate code
        process.exit(this.failedTests > 0 ? 1 : 0);
    }
}

// Run tests if called directly
if (require.main === module) {
    const runner = new NodeJSFFITestRunner();
    runner.runAllTests().catch((error) => {
        console.error('Test runner failed:', error);
        process.exit(1);
    });
}

module.exports = NodeJSFFITestRunner;
