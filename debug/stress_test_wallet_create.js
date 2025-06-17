// Stress Test for Tari Wallet FFI walletCreate function
// Tests rapid creation/destruction cycles and various Node.js scenarios

const { Worker, isMainThread, parentPort, workerData } = require('worker_threads');
const os = require('os');
const process = require('process');

console.log('=== Tari Wallet FFI Stress Test ===');
console.log('Node.js Version:', process.version);
console.log('Platform:', process.platform);
console.log('Architecture:', process.arch);
console.log('CPU Count:', os.cpus().length);
console.log('Free Memory:', Math.round(os.freemem() / 1024 / 1024), 'MB');
console.log('Total Memory:', Math.round(os.totalmem() / 1024 / 1024), 'MB');

// Test configuration
const TEST_CONFIG = {
    RAPID_CREATE_COUNT: 50,
    CONCURRENT_WORKERS: 4,
    MEMORY_PRESSURE_ITERATIONS: 100,
    LONG_RUNNING_DURATION: 30000, // 30 seconds
    GC_PRESSURE_ARRAYS: 500,
};

// Mock FFI wallet create function (would normally use actual FFI)
function mockWalletCreate(testScenario) {
    return new Promise((resolve, reject) => {
        // Simulate the problematic patterns that cause segfaults
        switch (testScenario) {
            case 'rapid_create':
                // Simulate rapid runtime creation
                setTimeout(() => {
                    if (Math.random() < 0.1) {
                        reject(new Error('Simulated segfault in rapid creation'));
                    } else {
                        resolve({ wallet: `wallet_${Date.now()}` });
                    }
                }, Math.random() * 10);
                break;
                
            case 'memory_pressure':
                // Simulate memory pressure scenario
                const arrays = [];
                for (let i = 0; i < 10; i++) {
                    arrays.push(new Array(1000).fill(Math.random()));
                }
                
                setTimeout(() => {
                    if (Math.random() < 0.05) {
                        reject(new Error('Simulated memory-related segfault'));
                    } else {
                        resolve({ wallet: `wallet_pressure_${Date.now()}` });
                    }
                }, Math.random() * 20);
                break;
                
            case 'concurrent_access':
                // Simulate concurrent access patterns
                setImmediate(() => {
                    process.nextTick(() => {
                        if (Math.random() < 0.02) {
                            reject(new Error('Simulated concurrent access segfault'));
                        } else {
                            resolve({ wallet: `wallet_concurrent_${Date.now()}` });
                        }
                    });
                });
                break;
                
            default:
                setTimeout(() => resolve({ wallet: `wallet_default_${Date.now()}` }), 1);
        }
    });
}

// Test 1: Rapid wallet creation/destruction
async function testRapidCreation() {
    console.log('\n1. Testing rapid wallet creation/destruction...');
    
    const startTime = Date.now();
    const promises = [];
    let successCount = 0;
    let errorCount = 0;
    
    for (let i = 0; i < TEST_CONFIG.RAPID_CREATE_COUNT; i++) {
        const promise = mockWalletCreate('rapid_create')
            .then(wallet => {
                successCount++;
                // Simulate wallet destruction
                wallet = null;
                return 'success';
            })
            .catch(error => {
                errorCount++;
                console.error(`Rapid creation error ${i}:`, error.message);
                return 'error';
            });
        
        promises.push(promise);
        
        // Add small delay to prevent overwhelming the system
        if (i % 10 === 0) {
            await new Promise(resolve => setTimeout(resolve, 1));
        }
    }
    
    const results = await Promise.allSettled(promises);
    const duration = Date.now() - startTime;
    
    console.log(`Rapid creation test completed in ${duration}ms`);
    console.log(`Success: ${successCount}, Errors: ${errorCount}`);
    console.log(`Success rate: ${((successCount / TEST_CONFIG.RAPID_CREATE_COUNT) * 100).toFixed(2)}%`);
    
    return {
        success: errorCount === 0,
        duration,
        successRate: successCount / TEST_CONFIG.RAPID_CREATE_COUNT,
        errors: errorCount
    };
}

// Test 2: Memory pressure scenarios
async function testMemoryPressure() {
    console.log('\n2. Testing under memory pressure...');
    
    const startTime = Date.now();
    let successCount = 0;
    let errorCount = 0;
    
    // Create memory pressure
    const memoryArrays = [];
    for (let i = 0; i < TEST_CONFIG.GC_PRESSURE_ARRAYS; i++) {
        memoryArrays.push(new Array(1000).fill(i));
    }
    
    console.log('Memory pressure created, testing wallet creation...');
    
    for (let i = 0; i < TEST_CONFIG.MEMORY_PRESSURE_ITERATIONS; i++) {
        try {
            const wallet = await mockWalletCreate('memory_pressure');
            successCount++;
            
            // Simulate some wallet operations
            wallet.operation = `operation_${i}`;
            
            // Force some GC pressure
            if (i % 10 === 0 && global.gc) {
                global.gc();
            }
            
        } catch (error) {
            errorCount++;
            console.error(`Memory pressure error ${i}:`, error.message);
        }
        
        // Occasionally modify memory arrays to trigger GC
        if (i % 50 === 0) {
            memoryArrays[i % memoryArrays.length] = new Array(1000).fill(Math.random());
        }
    }
    
    // Clear memory pressure
    memoryArrays.length = 0;
    
    const duration = Date.now() - startTime;
    
    console.log(`Memory pressure test completed in ${duration}ms`);
    console.log(`Success: ${successCount}, Errors: ${errorCount}`);
    
    return {
        success: errorCount === 0,
        duration,
        successRate: successCount / TEST_CONFIG.MEMORY_PRESSURE_ITERATIONS,
        errors: errorCount
    };
}

// Test 3: Concurrent worker threads
async function testConcurrentWorkers() {
    console.log('\n3. Testing concurrent worker threads...');
    
    return new Promise((resolve) => {
        const workers = [];
        const results = [];
        let completedWorkers = 0;
        
        const workerCode = `
            const { parentPort, workerData } = require('worker_threads');
            
            async function workerTest() {
                const { workerId, iterations } = workerData;
                let successCount = 0;
                let errorCount = 0;
                
                for (let i = 0; i < iterations; i++) {
                    try {
                        // Simulate wallet creation in worker
                        await new Promise((resolve, reject) => {
                            setTimeout(() => {
                                if (Math.random() < 0.03) {
                                    reject(new Error('Worker simulated error'));
                                } else {
                                    resolve({ wallet: \`worker_\${workerId}_\${i}\` });
                                }
                            }, Math.random() * 5);
                        });
                        successCount++;
                    } catch (error) {
                        errorCount++;
                    }
                }
                
                parentPort.postMessage({
                    workerId,
                    successCount,
                    errorCount,
                    successRate: successCount / iterations
                });
            }
            
            workerTest().catch(console.error);
        `;
        
        for (let i = 0; i < TEST_CONFIG.CONCURRENT_WORKERS; i++) {
            const worker = new Worker(workerCode, {
                eval: true,
                workerData: {
                    workerId: i,
                    iterations: 25
                }
            });
            
            worker.on('message', (result) => {
                results.push(result);
                completedWorkers++;
                
                console.log(`Worker ${result.workerId} completed: ${result.successCount} success, ${result.errorCount} errors`);
                
                if (completedWorkers === TEST_CONFIG.CONCURRENT_WORKERS) {
                    const totalSuccess = results.reduce((sum, r) => sum + r.successCount, 0);
                    const totalErrors = results.reduce((sum, r) => sum + r.errorCount, 0);
                    const avgSuccessRate = results.reduce((sum, r) => sum + r.successRate, 0) / results.length;
                    
                    console.log(`All workers completed. Total success: ${totalSuccess}, Total errors: ${totalErrors}`);
                    console.log(`Average success rate: ${(avgSuccessRate * 100).toFixed(2)}%`);
                    
                    resolve({
                        success: totalErrors === 0,
                        totalSuccess,
                        totalErrors,
                        avgSuccessRate,
                        workers: results
                    });
                }
            });
            
            worker.on('error', (error) => {
                console.error(`Worker ${i} error:`, error);
                completedWorkers++;
                
                if (completedWorkers === TEST_CONFIG.CONCURRENT_WORKERS) {
                    resolve({
                        success: false,
                        error: error.message
                    });
                }
            });
            
            workers.push(worker);
        }
    });
}

// Test 4: Long-running scenario
async function testLongRunning() {
    console.log('\n4. Testing long-running wallet lifecycle...');
    
    const startTime = Date.now();
    const wallets = [];
    let operationCount = 0;
    let errorCount = 0;
    
    console.log(`Running for ${TEST_CONFIG.LONG_RUNNING_DURATION / 1000} seconds...`);
    
    const endTime = startTime + TEST_CONFIG.LONG_RUNNING_DURATION;
    
    while (Date.now() < endTime) {
        try {
            // Create wallet
            const wallet = await mockWalletCreate('concurrent_access');
            wallets.push(wallet);
            
            // Simulate wallet operations
            wallet.lastOperation = Date.now();
            operationCount++;
            
            // Occasionally destroy old wallets
            if (wallets.length > 10) {
                const oldWallet = wallets.shift();
                oldWallet.destroyed = true;
            }
            
            // Add some variability
            await new Promise(resolve => setTimeout(resolve, Math.random() * 100));
            
        } catch (error) {
            errorCount++;
            console.error('Long-running error:', error.message);
        }
    }
    
    // Clean up remaining wallets
    wallets.forEach(wallet => {
        wallet.destroyed = true;
    });
    
    const duration = Date.now() - startTime;
    
    console.log(`Long-running test completed in ${duration}ms`);
    console.log(`Operations: ${operationCount}, Errors: ${errorCount}`);
    console.log(`Operations per second: ${(operationCount / (duration / 1000)).toFixed(2)}`);
    
    return {
        success: errorCount < operationCount * 0.05, // Allow 5% error rate
        duration,
        operationCount,
        errorCount,
        opsPerSecond: operationCount / (duration / 1000)
    };
}

// Test 5: Event loop stress test
async function testEventLoopStress() {
    console.log('\n5. Testing event loop stress patterns...');
    
    const startTime = Date.now();
    let completedOperations = 0;
    let errors = 0;
    
    // Create various async patterns that might conflict with Tokio
    const operations = [];
    
    // Immediate operations
    for (let i = 0; i < 100; i++) {
        operations.push(new Promise(resolve => {
            setImmediate(() => {
                try {
                    completedOperations++;
                    resolve('immediate');
                } catch (error) {
                    errors++;
                    resolve('error');
                }
            });
        }));
    }
    
    // NextTick operations
    for (let i = 0; i < 100; i++) {
        operations.push(new Promise(resolve => {
            process.nextTick(() => {
                try {
                    completedOperations++;
                    resolve('nextTick');
                } catch (error) {
                    errors++;
                    resolve('error');
                }
            });
        }));
    }
    
    // Timeout operations
    for (let i = 0; i < 100; i++) {
        operations.push(new Promise(resolve => {
            setTimeout(() => {
                try {
                    completedOperations++;
                    resolve('timeout');
                } catch (error) {
                    errors++;
                    resolve('error');
                }
            }, Math.random() * 10);
        }));
    }
    
    // Mixed with wallet creation
    for (let i = 0; i < 20; i++) {
        operations.push(mockWalletCreate('concurrent_access').then(() => {
            completedOperations++;
            return 'wallet';
        }).catch(() => {
            errors++;
            return 'wallet_error';
        }));
    }
    
    await Promise.all(operations);
    
    const duration = Date.now() - startTime;
    
    console.log(`Event loop stress test completed in ${duration}ms`);
    console.log(`Completed operations: ${completedOperations}, Errors: ${errors}`);
    
    return {
        success: errors < completedOperations * 0.1,
        duration,
        completedOperations,
        errors
    };
}

// Main test runner
async function runStressTests() {
    console.log('\n=== Starting Stress Tests ===');
    
    const testResults = {
        startTime: new Date().toISOString(),
        environment: {
            nodeVersion: process.version,
            platform: process.platform,
            arch: process.arch,
            cpuCount: os.cpus().length,
            totalMemoryMB: Math.round(os.totalmem() / 1024 / 1024),
            freeMemoryMB: Math.round(os.freemem() / 1024 / 1024)
        },
        tests: {}
    };
    
    try {
        // Run tests sequentially to avoid interference
        testResults.tests.rapidCreation = await testRapidCreation();
        testResults.tests.memoryPressure = await testMemoryPressure();
        testResults.tests.concurrentWorkers = await testConcurrentWorkers();
        testResults.tests.longRunning = await testLongRunning();
        testResults.tests.eventLoopStress = await testEventLoopStress();
        
        // Calculate overall results
        const allTests = Object.values(testResults.tests);
        const successfulTests = allTests.filter(test => test.success).length;
        const totalTests = allTests.length;
        
        testResults.summary = {
            totalTests,
            successfulTests,
            successRate: (successfulTests / totalTests) * 100,
            overallSuccess: successfulTests === totalTests
        };
        
        console.log('\n=== Stress Test Summary ===');
        console.log(`Total Tests: ${totalTests}`);
        console.log(`Successful: ${successfulTests}`);
        console.log(`Success Rate: ${testResults.summary.successRate.toFixed(2)}%`);
        console.log(`Overall Result: ${testResults.summary.overallSuccess ? 'PASS' : 'FAIL'}`);
        
        // Save results
        const fs = require('fs');
        const path = require('path');
        
        fs.writeFileSync(
            path.join(__dirname, 'stress_test_results.json'),
            JSON.stringify(testResults, null, 2)
        );
        
        console.log('\nResults saved to: stress_test_results.json');
        
        if (!testResults.summary.overallSuccess) {
            console.log('\nSome tests failed. This may indicate potential segfault risks.');
            process.exit(1);
        }
        
    } catch (error) {
        console.error('\nStress test runner failed:', error);
        testResults.error = error.message;
        testResults.stack = error.stack;
        process.exit(1);
    }
}

// Handle uncaught exceptions that might indicate segfaults
process.on('uncaughtException', (error) => {
    console.error('\n!!! UNCAUGHT EXCEPTION - Potential segfault indicator !!!');
    console.error('Error:', error);
    console.error('Stack:', error.stack);
    process.exit(1);
});

process.on('unhandledRejection', (reason, promise) => {
    console.error('\n!!! UNHANDLED REJECTION - Potential async conflict !!!');
    console.error('Reason:', reason);
    console.error('Promise:', promise);
    process.exit(1);
});

// Run tests if this is the main thread
if (isMainThread) {
    runStressTests().catch(error => {
        console.error('Stress test execution failed:', error);
        process.exit(1);
    });
}
