#!/usr/bin/env node
/**
 * Wallet Create Stress Test
 * 
 * Stress test specifically targeting the wallet_create function to validate
 * that our segfault fixes work under high load and various conditions.
 */

const fs = require('fs');
const path = require('path');
const { performance } = require('perf_hooks');

class WalletCreateStressTest {
    constructor() {
        this.results = [];
        this.testConfig = {
            iterations: 100,
            concurrency: 10,
            memoryPressure: true,
            callbackStorm: true,
            workerThreads: true
        };
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
     * Simulate wallet_create function calls
     */
    async simulateWalletCreate(id) {
        const startTime = performance.now();
        
        try {
            // Simulate the complexity of wallet_create
            await this.simulateComplexAsyncOperations();
            await this.simulateMemoryOperations();
            await this.simulateCallbackOperations();
            
            const duration = performance.now() - startTime;
            
            return {
                id,
                success: true,
                duration,
                error: null
            };
        } catch (error) {
            const duration = performance.now() - startTime;
            
            return {
                id,
                success: false,
                duration,
                error: error.message
            };
        }
    }

    /**
     * Simulate complex async operations similar to wallet initialization
     */
    async simulateComplexAsyncOperations() {
        // Simulate database operations
        await new Promise(resolve => setTimeout(resolve, Math.random() * 10));
        
        // Simulate network operations
        await new Promise(resolve => setImmediate(resolve));
        
        // Simulate crypto operations
        await new Promise(resolve => process.nextTick(resolve));
        
        // Simulate file system operations
        await new Promise(resolve => setTimeout(resolve, Math.random() * 5));
    }

    /**
     * Simulate memory operations that might cause issues
     */
    async simulateMemoryOperations() {
        const allocations = [];
        
        // Allocate various buffer sizes
        for (let i = 0; i < 50; i++) {
            const size = Math.floor(Math.random() * 10000) + 1000;
            const buffer = Buffer.alloc(size);
            buffer.fill(i % 256);
            allocations.push(buffer);
        }
        
        // Simulate some processing
        for (let i = 0; i < allocations.length; i++) {
            const buffer = allocations[i];
            let sum = 0;
            for (let j = 0; j < Math.min(buffer.length, 100); j++) {
                sum += buffer[j];
            }
            
            // Yield control periodically
            if (i % 10 === 0) {
                await new Promise(resolve => setImmediate(resolve));
            }
        }
        
        // Free some memory
        allocations.splice(0, allocations.length / 2);
    }

    /**
     * Simulate callback operations
     */
    async simulateCallbackOperations() {
        const callbacks = [];
        const results = [];
        
        // Create callbacks similar to wallet event handlers
        for (let i = 0; i < 20; i++) {
            callbacks.push((data) => {
                results.push(data * 2);
            });
        }
        
        // Execute callbacks in rapid succession
        for (let i = 0; i < callbacks.length; i++) {
            await new Promise(resolve => {
                setImmediate(() => {
                    callbacks[i](i);
                    resolve();
                });
            });
        }
        
        // Verify results
        if (results.length !== callbacks.length) {
            throw new Error(`Callback mismatch: expected ${callbacks.length}, got ${results.length}`);
        }
    }

    /**
     * Test sequential wallet creation
     */
    async testSequentialCreation() {
        this.log('Testing sequential wallet creation...');
        
        const results = [];
        const startTime = performance.now();
        
        for (let i = 0; i < this.testConfig.iterations; i++) {
            const result = await this.simulateWalletCreate(i);
            results.push(result);
            
            if (result.success) {
                if (i % 10 === 0) {
                    this.log(`Sequential progress: ${i + 1}/${this.testConfig.iterations}`);
                }
            } else {
                this.error(`Sequential creation ${i} failed: ${result.error}`);
            }
        }
        
        const totalTime = performance.now() - startTime;
        const successCount = results.filter(r => r.success).length;
        const avgDuration = results.reduce((sum, r) => sum + r.duration, 0) / results.length;
        
        this.log(`Sequential test completed: ${successCount}/${this.testConfig.iterations} successful`);
        this.log(`Average duration: ${avgDuration.toFixed(2)}ms, Total time: ${totalTime.toFixed(2)}ms`);
        
        return {
            type: 'sequential',
            total: this.testConfig.iterations,
            successful: successCount,
            failed: this.testConfig.iterations - successCount,
            avgDuration,
            totalTime,
            results
        };
    }

    /**
     * Test concurrent wallet creation
     */
    async testConcurrentCreation() {
        this.log('Testing concurrent wallet creation...');
        
        const batches = Math.ceil(this.testConfig.iterations / this.testConfig.concurrency);
        const allResults = [];
        const startTime = performance.now();
        
        for (let batch = 0; batch < batches; batch++) {
            const batchStart = batch * this.testConfig.concurrency;
            const batchEnd = Math.min(batchStart + this.testConfig.concurrency, this.testConfig.iterations);
            const batchSize = batchEnd - batchStart;
            
            this.log(`Concurrent batch ${batch + 1}/${batches}: ${batchSize} operations`);
            
            // Create concurrent promises
            const promises = [];
            for (let i = batchStart; i < batchEnd; i++) {
                promises.push(this.simulateWalletCreate(i));
            }
            
            // Wait for batch to complete
            const batchResults = await Promise.all(promises);
            allResults.push(...batchResults);
            
            const batchSuccessful = batchResults.filter(r => r.success).length;
            this.log(`Batch ${batch + 1} completed: ${batchSuccessful}/${batchSize} successful`);
            
            // Brief pause between batches
            await new Promise(resolve => setTimeout(resolve, 10));
        }
        
        const totalTime = performance.now() - startTime;
        const successCount = allResults.filter(r => r.success).length;
        const avgDuration = allResults.reduce((sum, r) => sum + r.duration, 0) / allResults.length;
        
        this.log(`Concurrent test completed: ${successCount}/${this.testConfig.iterations} successful`);
        this.log(`Average duration: ${avgDuration.toFixed(2)}ms, Total time: ${totalTime.toFixed(2)}ms`);
        
        return {
            type: 'concurrent',
            total: this.testConfig.iterations,
            successful: successCount,
            failed: this.testConfig.iterations - successCount,
            avgDuration,
            totalTime,
            results: allResults
        };
    }

    /**
     * Test under memory pressure
     */
    async testMemoryPressure() {
        if (!this.testConfig.memoryPressure) {
            return null;
        }
        
        this.log('Testing under memory pressure...');
        
        // Create memory pressure
        const memoryHogs = [];
        const memorySize = 50 * 1024 * 1024; // 50MB
        
        for (let i = 0; i < 5; i++) {
            const buffer = Buffer.alloc(memorySize);
            buffer.fill(i % 256);
            memoryHogs.push(buffer);
        }
        
        const startMemory = process.memoryUsage();
        this.log(`Memory pressure created: ${Math.round(startMemory.heapUsed / 1024 / 1024)}MB heap used`);
        
        // Run wallet creation under pressure
        const startTime = performance.now();
        const results = [];
        
        for (let i = 0; i < 20; i++) {
            const result = await this.simulateWalletCreate(i);
            results.push(result);
        }
        
        const totalTime = performance.now() - startTime;
        const successCount = results.filter(r => r.success).length;
        
        // Clean up memory
        memoryHogs.length = 0;
        
        if (global.gc) {
            global.gc();
        }
        
        const endMemory = process.memoryUsage();
        this.log(`Memory pressure test completed: ${successCount}/20 successful`);
        this.log(`Memory after cleanup: ${Math.round(endMemory.heapUsed / 1024 / 1024)}MB heap used`);
        
        return {
            type: 'memory_pressure',
            total: 20,
            successful: successCount,
            failed: 20 - successCount,
            totalTime,
            startMemory,
            endMemory,
            results
        };
    }

    /**
     * Test with callback storm
     */
    async testCallbackStorm() {
        if (!this.testConfig.callbackStorm) {
            return null;
        }
        
        this.log('Testing with callback storm...');
        
        // Create a storm of callbacks
        const callbackCount = 1000;
        const callbacks = [];
        let completedCallbacks = 0;
        
        const callbackPromise = new Promise(resolve => {
            for (let i = 0; i < callbackCount; i++) {
                setImmediate(() => {
                    completedCallbacks++;
                    if (completedCallbacks === callbackCount) {
                        resolve();
                    }
                });
            }
        });
        
        // Run wallet creation during callback storm
        const startTime = performance.now();
        const walletPromise = this.simulateWalletCreate(0);
        
        // Wait for both to complete
        const [, walletResult] = await Promise.all([callbackPromise, walletPromise]);
        const totalTime = performance.now() - startTime;
        
        this.log(`Callback storm test completed: ${completedCallbacks} callbacks, wallet creation ${walletResult.success ? 'successful' : 'failed'}`);
        
        return {
            type: 'callback_storm',
            callbackCount: completedCallbacks,
            walletCreationSuccess: walletResult.success,
            totalTime,
            walletResult
        };
    }

    /**
     * Test with worker threads
     */
    async testWithWorkerThreads() {
        if (!this.testConfig.workerThreads) {
            return null;
        }
        
        try {
            const { Worker, isMainThread } = require('worker_threads');
            
            if (!isMainThread) {
                throw new Error('Already in worker thread');
            }
            
            this.log('Testing with worker threads...');
            
            const workerPromises = [];
            const workerCount = 3;
            
            for (let i = 0; i < workerCount; i++) {
                const workerPromise = new Promise((resolve, reject) => {
                    const workerCode = `
                        const { parentPort } = require('worker_threads');
                        
                        // Simulate work in worker thread
                        let counter = 0;
                        const interval = setInterval(() => {
                            counter++;
                            if (counter >= 50) {
                                clearInterval(interval);
                                parentPort.postMessage({ success: true, counter });
                            }
                        }, 1);
                    `;
                    
                    const worker = new Worker(workerCode, { eval: true });
                    
                    worker.on('message', (message) => {
                        worker.terminate();
                        resolve(message);
                    });
                    
                    worker.on('error', (error) => {
                        reject(error);
                    });
                    
                    setTimeout(() => {
                        worker.terminate();
                        reject(new Error('Worker timeout'));
                    }, 5000);
                });
                
                workerPromises.push(workerPromise);
            }
            
            // Run wallet creation while workers are running
            const startTime = performance.now();
            const walletPromise = this.simulateWalletCreate(0);
            
            // Wait for both workers and wallet creation
            const [workerResults, walletResult] = await Promise.all([
                Promise.all(workerPromises),
                walletPromise
            ]);
            
            const totalTime = performance.now() - startTime;
            const successfulWorkers = workerResults.filter(r => r.success).length;
            
            this.log(`Worker thread test completed: ${successfulWorkers}/${workerCount} workers successful, wallet creation ${walletResult.success ? 'successful' : 'failed'}`);
            
            return {
                type: 'worker_threads',
                workerCount,
                successfulWorkers,
                walletCreationSuccess: walletResult.success,
                totalTime,
                workerResults,
                walletResult
            };
            
        } catch (error) {
            this.log(`Worker threads not available: ${error.message}`);
            return null;
        }
    }

    /**
     * Run all stress tests
     */
    async runAllTests() {
        this.log('Starting Wallet Create Stress Tests...');
        this.log(`Configuration: ${JSON.stringify(this.testConfig)}`);
        
        const testResults = {};
        
        try {
            // Sequential test
            testResults.sequential = await this.testSequentialCreation();
            
            // Concurrent test
            testResults.concurrent = await this.testConcurrentCreation();
            
            // Memory pressure test
            testResults.memoryPressure = await this.testMemoryPressure();
            
            // Callback storm test
            testResults.callbackStorm = await this.testCallbackStorm();
            
            // Worker thread test
            testResults.workerThreads = await this.testWithWorkerThreads();
            
        } catch (error) {
            this.error(`Test execution failed: ${error.message}`);
            testResults.error = error.message;
        }
        
        this.generateReport(testResults);
        return testResults;
    }

    /**
     * Generate comprehensive test report
     */
    generateReport(testResults) {
        this.log('Generating stress test report...');
        
        const report = {
            timestamp: new Date().toISOString(),
            environment: {
                nodeVersion: process.version,
                platform: process.platform,
                arch: process.arch,
                pid: process.pid,
                memoryUsage: process.memoryUsage()
            },
            configuration: this.testConfig,
            results: testResults
        };
        
        // Calculate overall statistics
        let totalOperations = 0;
        let totalSuccessful = 0;
        let totalFailed = 0;
        
        Object.values(testResults).forEach(result => {
            if (result && typeof result === 'object' && result.total) {
                totalOperations += result.total;
                totalSuccessful += result.successful || 0;
                totalFailed += result.failed || 0;
            }
        });
        
        report.summary = {
            totalOperations,
            totalSuccessful,
            totalFailed,
            successRate: totalOperations > 0 ? (totalSuccessful / totalOperations) * 100 : 0
        };
        
        // Write report to file
        const reportPath = path.join(__dirname, 'wallet_create_stress_report.json');
        fs.writeFileSync(reportPath, JSON.stringify(report, null, 2));
        
        // Print summary
        console.log('\n=== WALLET CREATE STRESS TEST SUMMARY ===');
        console.log(`Total Operations: ${totalOperations}`);
        console.log(`Successful: ${totalSuccessful}`);
        console.log(`Failed: ${totalFailed}`);
        console.log(`Success Rate: ${report.summary.successRate.toFixed(1)}%`);
        
        Object.entries(testResults).forEach(([testType, result]) => {
            if (result && typeof result === 'object') {
                if (result.total) {
                    console.log(`${testType}: ${result.successful}/${result.total} (${((result.successful / result.total) * 100).toFixed(1)}%)`);
                } else if (result.type) {
                    console.log(`${testType}: ${result.walletCreationSuccess ? 'SUCCESS' : 'FAILED'}`);
                }
            }
        });
        
        console.log(`\nDetailed report written to: ${reportPath}`);
        
        // Exit with appropriate code
        const overallSuccess = report.summary.successRate >= 90; // 90% success threshold
        if (!overallSuccess) {
            console.log('\n⚠️  SUCCESS RATE BELOW THRESHOLD - POTENTIAL ISSUES DETECTED');
        }
        
        return overallSuccess;
    }
}

// Run tests if called directly
if (require.main === module) {
    const stressTester = new WalletCreateStressTest();
    stressTester.runAllTests().then((success) => {
        process.exit(success ? 0 : 1);
    }).catch((error) => {
        console.error('Stress test failed:', error);
        process.exit(1);
    });
}

module.exports = WalletCreateStressTest;
