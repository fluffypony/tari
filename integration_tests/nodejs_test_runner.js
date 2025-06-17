// Node.js Integration Test Runner for Tari Wallet FFI
// Tests actual FFI integration with Node.js runtime

const { spawn } = require('child_process');
const fs = require('fs');
const path = require('path');
const os = require('os');

console.log('=== Tari Wallet FFI Node.js Integration Tests ===');

// Test configuration
const TEST_CONFIG = {
    CARGO_TARGET_DIR: process.env.CARGO_TARGET_DIR || path.join(__dirname, '..', 'target'),
    DEBUG_FEATURES: ['debug_runtime', 'debug_memory', 'nodejs_compatibility'],
    TEST_TIMEOUT: 60000, // 60 seconds
    FFI_LIB_NAME: process.platform === 'win32' ? 'minotari_wallet_ffi.dll' : 
                  process.platform === 'darwin' ? 'libminotari_wallet_ffi.dylib' : 
                  'libminotari_wallet_ffi.so'
};

// Helper function to run cargo commands
function runCargo(args, options = {}) {
    return new Promise((resolve, reject) => {
        console.log(`Running: cargo ${args.join(' ')}`);
        
        const cargo = spawn('cargo', args, {
            stdio: ['inherit', 'pipe', 'pipe'],
            cwd: path.join(__dirname, '..', 'base_layer', 'wallet_ffi'),
            ...options
        });
        
        let stdout = '';
        let stderr = '';
        
        cargo.stdout.on('data', (data) => {
            const output = data.toString();
            stdout += output;
            console.log(output.trim());
        });
        
        cargo.stderr.on('data', (data) => {
            const output = data.toString();
            stderr += output;
            console.error(output.trim());
        });
        
        cargo.on('close', (code) => {
            if (code === 0) {
                resolve({ stdout, stderr });
            } else {
                reject(new Error(`Cargo command failed with code ${code}`));
            }
        });
        
        // Set timeout
        setTimeout(() => {
            cargo.kill('SIGTERM');
            reject(new Error('Cargo command timed out'));
        }, TEST_CONFIG.TEST_TIMEOUT);
    });
}

// Test 1: Build FFI library with debug features
async function testBuildWithDebugFeatures() {
    console.log('\n1. Building FFI library with debug features...');
    
    const features = TEST_CONFIG.DEBUG_FEATURES.join(',');
    
    try {
        await runCargo(['build', '--release', '--features', features]);
        
        // Check if library was built
        const libPath = path.join(
            TEST_CONFIG.CARGO_TARGET_DIR,
            'release',
            TEST_CONFIG.FFI_LIB_NAME
        );
        
        if (fs.existsSync(libPath)) {
            const stats = fs.statSync(libPath);
            console.log(`✓ FFI library built successfully: ${libPath}`);
            console.log(`  Size: ${Math.round(stats.size / 1024 / 1024)} MB`);
            console.log(`  Modified: ${stats.mtime.toISOString()}`);
            return { success: true, libPath, size: stats.size };
        } else {
            throw new Error(`FFI library not found at expected path: ${libPath}`);
        }
        
    } catch (error) {
        console.error('✗ FFI library build failed:', error.message);
        return { success: false, error: error.message };
    }
}

// Test 2: Build without debug features (baseline)
async function testBuildBaseline() {
    console.log('\n2. Building FFI library without debug features (baseline)...');
    
    try {
        await runCargo(['build', '--release']);
        
        const libPath = path.join(
            TEST_CONFIG.CARGO_TARGET_DIR,
            'release',
            TEST_CONFIG.FFI_LIB_NAME
        );
        
        if (fs.existsSync(libPath)) {
            const stats = fs.statSync(libPath);
            console.log(`✓ Baseline FFI library built successfully`);
            console.log(`  Size: ${Math.round(stats.size / 1024 / 1024)} MB`);
            return { success: true, libPath, size: stats.size };
        } else {
            throw new Error(`Baseline FFI library not found`);
        }
        
    } catch (error) {
        console.error('✗ Baseline FFI library build failed:', error.message);
        return { success: false, error: error.message };
    }
}

// Test 3: Run Rust unit tests with debug features
async function testRustUnitTests() {
    console.log('\n3. Running Rust unit tests with debug features...');
    
    const features = TEST_CONFIG.DEBUG_FEATURES.join(',');
    
    try {
        await runCargo(['test', '--features', features]);
        console.log('✓ Rust unit tests passed');
        return { success: true };
        
    } catch (error) {
        console.error('✗ Rust unit tests failed:', error.message);
        return { success: false, error: error.message };
    }
}

// Test 4: Test FFI library loading in Node.js
async function testFFILoading() {
    console.log('\n4. Testing FFI library loading in Node.js...');
    
    try {
        // Create a simple test script that loads the FFI
        const testScript = `
            const ffi = require('ffi-napi');
            const path = require('path');
            
            const libPath = '${path.join(TEST_CONFIG.CARGO_TARGET_DIR, 'release', TEST_CONFIG.FFI_LIB_NAME)}';
            
            console.log('Attempting to load FFI library:', libPath);
            
            try {
                const lib = ffi.Library(libPath, {
                    // These are some basic exported functions from the FFI
                    'string_create': ['pointer', ['string']],
                    'string_destroy': ['void', ['pointer']],
                });
                
                console.log('✓ FFI library loaded successfully');
                console.log('Available functions:', Object.keys(lib));
                
                // Test basic string operations
                const testStr = lib.string_create('test_string');
                if (testStr && !testStr.isNull()) {
                    console.log('✓ Basic FFI function call successful');
                    lib.string_destroy(testStr);
                    console.log('✓ Memory cleanup successful');
                } else {
                    throw new Error('FFI function returned null');
                }
                
                process.exit(0);
                
            } catch (error) {
                console.error('✗ FFI library loading failed:', error.message);
                process.exit(1);
            }
        `;
        
        // Write test script to temporary file
        const testScriptPath = path.join(os.tmpdir(), 'tari_ffi_test.js');
        fs.writeFileSync(testScriptPath, testScript);
        
        // Run the test script in a separate Node.js process
        const result = await new Promise((resolve, reject) => {
            const node = spawn('node', [testScriptPath], {
                stdio: ['inherit', 'pipe', 'pipe'],
                timeout: 30000
            });
            
            let stdout = '';
            let stderr = '';
            
            node.stdout.on('data', (data) => {
                const output = data.toString();
                stdout += output;
                console.log(output.trim());
            });
            
            node.stderr.on('data', (data) => {
                const output = data.toString();
                stderr += output;
                console.error(output.trim());
            });
            
            node.on('close', (code) => {
                // Clean up test script
                try {
                    fs.unlinkSync(testScriptPath);
                } catch (e) {
                    // Ignore cleanup errors
                }
                
                if (code === 0) {
                    resolve({ success: true, stdout, stderr });
                } else {
                    resolve({ success: false, code, stdout, stderr });
                }
            });
            
            setTimeout(() => {
                node.kill('SIGTERM');
                reject(new Error('FFI loading test timed out'));
            }, 30000);
        });
        
        return result;
        
    } catch (error) {
        console.error('✗ FFI loading test failed:', error.message);
        return { success: false, error: error.message };
    }
}

// Test 5: Memory leak detection
async function testMemoryLeaks() {
    console.log('\n5. Testing for memory leaks...');
    
    try {
        // Run a stress test that creates and destroys many FFI objects
        const testScript = `
            const ffi = require('ffi-napi');
            const path = require('path');
            
            const libPath = '${path.join(TEST_CONFIG.CARGO_TARGET_DIR, 'release', TEST_CONFIG.FFI_LIB_NAME)}';
            
            const lib = ffi.Library(libPath, {
                'string_create': ['pointer', ['string']],
                'string_destroy': ['void', ['pointer']],
            });
            
            const initialMemory = process.memoryUsage();
            console.log('Initial memory usage:', initialMemory);
            
            // Create and destroy many strings
            for (let i = 0; i < 1000; i++) {
                const str = lib.string_create(\`test_string_\${i}\`);
                if (str && !str.isNull()) {
                    lib.string_destroy(str);
                } else {
                    console.error('String creation failed at iteration', i);
                    process.exit(1);
                }
                
                if (i % 100 === 0) {
                    if (global.gc) global.gc();
                    const currentMemory = process.memoryUsage();
                    console.log(\`Memory at iteration \${i}:\`, currentMemory);
                }
            }
            
            if (global.gc) global.gc();
            const finalMemory = process.memoryUsage();
            console.log('Final memory usage:', finalMemory);
            
            const memoryIncrease = finalMemory.rss - initialMemory.rss;
            console.log('Memory increase:', memoryIncrease, 'bytes');
            
            // If memory increased by more than 10MB, consider it a potential leak
            if (memoryIncrease > 10 * 1024 * 1024) {
                console.error('Potential memory leak detected');
                process.exit(1);
            } else {
                console.log('✓ No significant memory leaks detected');
                process.exit(0);
            }
        `;
        
        const testScriptPath = path.join(os.tmpdir(), 'tari_memory_test.js');
        fs.writeFileSync(testScriptPath, testScript);
        
        const result = await new Promise((resolve, reject) => {
            const node = spawn('node', ['--expose-gc', testScriptPath], {
                stdio: ['inherit', 'pipe', 'pipe'],
                timeout: 60000
            });
            
            let stdout = '';
            let stderr = '';
            
            node.stdout.on('data', (data) => {
                const output = data.toString();
                stdout += output;
                console.log(output.trim());
            });
            
            node.stderr.on('data', (data) => {
                const output = data.toString();
                stderr += output;
                console.error(output.trim());
            });
            
            node.on('close', (code) => {
                try {
                    fs.unlinkSync(testScriptPath);
                } catch (e) {
                    // Ignore cleanup errors
                }
                
                resolve({ success: code === 0, code, stdout, stderr });
            });
            
            setTimeout(() => {
                node.kill('SIGTERM');
                reject(new Error('Memory leak test timed out'));
            }, 60000);
        });
        
        return result;
        
    } catch (error) {
        console.error('✗ Memory leak test failed:', error.message);
        return { success: false, error: error.message };
    }
}

// Main test runner
async function runIntegrationTests() {
    console.log('\n=== Starting Node.js Integration Tests ===');
    
    const testResults = {
        startTime: new Date().toISOString(),
        environment: {
            nodeVersion: process.version,
            platform: process.platform,
            arch: process.arch,
            cargoTargetDir: TEST_CONFIG.CARGO_TARGET_DIR,
            debugFeatures: TEST_CONFIG.DEBUG_FEATURES
        },
        tests: {}
    };
    
    try {
        // Check prerequisites
        console.log('\nChecking prerequisites...');
        
        // Check if ffi-napi is available
        try {
            require.resolve('ffi-napi');
            console.log('✓ ffi-napi is available');
        } catch (error) {
            console.log('✗ ffi-napi not found, trying to install...');
            console.log('Run: npm install ffi-napi');
            console.log('Note: This test will skip FFI loading tests');
        }
        
        // Run tests
        testResults.tests.buildWithDebugFeatures = await testBuildWithDebugFeatures();
        testResults.tests.buildBaseline = await testBuildBaseline();
        testResults.tests.rustUnitTests = await testRustUnitTests();
        
        // Only run FFI tests if library exists
        if (testResults.tests.buildWithDebugFeatures.success) {
            try {
                require.resolve('ffi-napi');
                testResults.tests.ffiLoading = await testFFILoading();
                testResults.tests.memoryLeaks = await testMemoryLeaks();
            } catch (error) {
                console.log('Skipping FFI tests due to missing ffi-napi');
                testResults.tests.ffiLoading = { success: true, skipped: true, reason: 'ffi-napi not available' };
                testResults.tests.memoryLeaks = { success: true, skipped: true, reason: 'ffi-napi not available' };
            }
        }
        
        // Calculate results
        const allTests = Object.values(testResults.tests);
        const successfulTests = allTests.filter(test => test.success).length;
        const skippedTests = allTests.filter(test => test.skipped).length;
        const totalTests = allTests.length;
        
        testResults.summary = {
            totalTests,
            successfulTests,
            skippedTests,
            failedTests: totalTests - successfulTests - skippedTests,
            successRate: (successfulTests / (totalTests - skippedTests)) * 100,
            overallSuccess: successfulTests === (totalTests - skippedTests)
        };
        
        console.log('\n=== Integration Test Summary ===');
        console.log(`Total Tests: ${totalTests}`);
        console.log(`Successful: ${successfulTests}`);
        console.log(`Skipped: ${skippedTests}`);
        console.log(`Failed: ${testResults.summary.failedTests}`);
        console.log(`Success Rate: ${testResults.summary.successRate.toFixed(2)}%`);
        console.log(`Overall Result: ${testResults.summary.overallSuccess ? 'PASS' : 'FAIL'}`);
        
        // Save results
        const resultsPath = path.join(__dirname, 'nodejs_integration_results.json');
        fs.writeFileSync(resultsPath, JSON.stringify(testResults, null, 2));
        console.log(`\nResults saved to: ${resultsPath}`);
        
        if (!testResults.summary.overallSuccess) {
            console.log('\nSome tests failed. Check the detailed results for more information.');
            process.exit(1);
        }
        
    } catch (error) {
        console.error('\nIntegration test runner failed:', error);
        testResults.error = error.message;
        process.exit(1);
    }
}

// Run tests
runIntegrationTests().catch(error => {
    console.error('Integration test execution failed:', error);
    process.exit(1);
});
