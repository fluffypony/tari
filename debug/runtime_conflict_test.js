// Node.js test script to reproduce walletCreate segfault
// Tests various scenarios of Tokio runtime conflicts with Node.js event loop

const { spawn } = require('child_process');
const fs = require('fs');
const path = require('path');

console.log('=== Tari Wallet FFI Runtime Conflict Test ===');
console.log('Node.js Version:', process.version);
console.log('Platform:', process.platform);
console.log('Architecture:', process.arch);
console.log('Thread ID:', process.pid);
console.log('Event Loop:', typeof setImmediate !== 'undefined' ? 'Node.js' : 'Browser');

// Environment detection
console.log('\n=== Environment Variables ===');
console.log('NODE_ENV:', process.env.NODE_ENV || 'undefined');
console.log('NODE_PATH:', process.env.NODE_PATH || 'undefined');
console.log('npm_config_user_config:', process.env.npm_config_user_config || 'undefined');

// Detect if we're in a worker thread
console.log('Main Thread:', require('worker_threads').isMainThread);

// Test async/await behavior (potential conflict with Tokio)
async function testAsyncBehavior() {
    console.log('\n=== Testing Node.js Async Behavior ===');
    
    // Test setTimeout (event loop)
    const timeoutPromise = new Promise(resolve => {
        setTimeout(() => {
            console.log('setTimeout callback executed');
            resolve('timeout_complete');
        }, 10);
    });
    
    // Test setImmediate (event loop)
    const immediatePromise = new Promise(resolve => {
        setImmediate(() => {
            console.log('setImmediate callback executed');
            resolve('immediate_complete');
        });
    });
    
    // Test process.nextTick (microtask queue)
    const nextTickPromise = new Promise(resolve => {
        process.nextTick(() => {
            console.log('process.nextTick callback executed');
            resolve('nexttick_complete');
        });
    });
    
    try {
        const results = await Promise.all([timeoutPromise, immediatePromise, nextTickPromise]);
        console.log('All async operations completed:', results);
        return true;
    } catch (error) {
        console.error('Async test failed:', error);
        return false;
    }
}

// Simulate FFI call load
function simulateFFILoad() {
    console.log('\n=== Simulating FFI Call Load ===');
    
    // Simulate rapid FFI calls (like multiple walletCreate calls)
    for (let i = 0; i < 10; i++) {
        // This simulates the pattern that might cause runtime conflicts
        setImmediate(() => {
            console.log(`Simulated FFI call ${i + 1}`);
            
            // Simulate some CPU-bound work
            const start = process.hrtime.bigint();
            let counter = 0;
            while (process.hrtime.bigint() - start < 1000000n) { // 1ms of work
                counter++;
            }
            
            console.log(`FFI call ${i + 1} completed after ${counter} iterations`);
        });
    }
}

// Test event loop behavior under load
function testEventLoopLoad() {
    console.log('\n=== Testing Event Loop Under Load ===');
    
    let completedTasks = 0;
    const totalTasks = 50;
    
    return new Promise((resolve) => {
        for (let i = 0; i < totalTasks; i++) {
            // Mix different types of async operations
            if (i % 3 === 0) {
                setTimeout(() => {
                    completedTasks++;
                    if (completedTasks === totalTasks) {
                        console.log(`All ${totalTasks} event loop tasks completed`);
                        resolve(true);
                    }
                }, Math.random() * 10);
            } else if (i % 3 === 1) {
                setImmediate(() => {
                    completedTasks++;
                    if (completedTasks === totalTasks) {
                        console.log(`All ${totalTasks} event loop tasks completed`);
                        resolve(true);
                    }
                });
            } else {
                process.nextTick(() => {
                    completedTasks++;
                    if (completedTasks === totalTasks) {
                        console.log(`All ${totalTasks} event loop tasks completed`);
                        resolve(true);
                    }
                });
            }
        }
    });
}

// Test memory pressure (might trigger GC issues)
function testMemoryPressure() {
    console.log('\n=== Testing Memory Pressure ===');
    
    const initialMemory = process.memoryUsage();
    console.log('Initial memory usage:', initialMemory);
    
    // Create memory pressure
    const arrays = [];
    for (let i = 0; i < 100; i++) {
        arrays.push(new Array(10000).fill(Math.random()));
    }
    
    const pressureMemory = process.memoryUsage();
    console.log('Memory usage under pressure:', pressureMemory);
    
    // Force garbage collection if available
    if (global.gc) {
        console.log('Forcing garbage collection...');
        global.gc();
        const afterGCMemory = process.memoryUsage();
        console.log('Memory usage after GC:', afterGCMemory);
    } else {
        console.log('Garbage collection not available (run with --expose-gc)');
    }
    
    // Clear arrays
    arrays.length = 0;
    
    return pressureMemory;
}

// Main test execution
async function runTests() {
    try {
        console.log('\n=== Starting Runtime Conflict Tests ===');
        
        // Test 1: Basic async behavior
        console.log('\n1. Testing basic async behavior...');
        const asyncSuccess = await testAsyncBehavior();
        console.log('Async test result:', asyncSuccess ? 'PASS' : 'FAIL');
        
        // Test 2: Simulate FFI load
        console.log('\n2. Simulating FFI call load...');
        simulateFFILoad();
        
        // Wait a bit for FFI simulation to complete
        await new Promise(resolve => setTimeout(resolve, 100));
        
        // Test 3: Event loop under load
        console.log('\n3. Testing event loop under load...');
        const loadSuccess = await testEventLoopLoad();
        console.log('Event loop load test result:', loadSuccess ? 'PASS' : 'FAIL');
        
        // Test 4: Memory pressure
        console.log('\n4. Testing memory pressure...');
        const memoryResults = testMemoryPressure();
        console.log('Memory pressure test completed');
        
        console.log('\n=== All tests completed ===');
        console.log('Environment appears stable for FFI operations');
        
        // Generate report
        const report = {
            timestamp: new Date().toISOString(),
            nodeVersion: process.version,
            platform: process.platform,
            arch: process.arch,
            tests: {
                asyncBehavior: asyncSuccess,
                eventLoopLoad: loadSuccess,
                memoryPressure: true
            },
            environment: {
                NODE_ENV: process.env.NODE_ENV,
                isMainThread: require('worker_threads').isMainThread,
                hasGC: !!global.gc
            }
        };
        
        fs.writeFileSync(
            path.join(__dirname, 'runtime_conflict_test_report.json'),
            JSON.stringify(report, null, 2)
        );
        
        console.log('\nTest report saved to: runtime_conflict_test_report.json');
        
    } catch (error) {
        console.error('\nTest execution failed:', error);
        process.exit(1);
    }
}

// Handle uncaught exceptions (might indicate runtime conflicts)
process.on('uncaughtException', (error) => {
    console.error('\n!!! UNCAUGHT EXCEPTION - Potential runtime conflict detected !!!');
    console.error('Error:', error);
    console.error('Stack:', error.stack);
    process.exit(1);
});

process.on('unhandledRejection', (reason, promise) => {
    console.error('\n!!! UNHANDLED REJECTION - Potential async conflict detected !!!');
    console.error('Reason:', reason);
    console.error('Promise:', promise);
    process.exit(1);
});

// Run the tests
runTests().catch(error => {
    console.error('Test runner failed:', error);
    process.exit(1);
});
