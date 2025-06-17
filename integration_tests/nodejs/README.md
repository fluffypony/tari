# Node.js Integration Tests for Tari Wallet FFI

This directory contains comprehensive Node.js integration tests for validating the segfault fixes and ensuring proper operation of the Tari wallet FFI in Node.js environments.

## Test Files

### `test_runner.js`
Comprehensive Node.js integration test runner that validates:
- Environment detection and Node.js compatibility
- Runtime conflict detection and resolution
- FFI library loading and basic functionality
- Memory pressure handling
- Async operation conflict scenarios
- Worker thread compatibility
- Callback pattern validation

### `ffi_tests.rs` 
Rust-side integration tests that validate:
- Debug infrastructure initialization
- Runtime strategy implementation
- Memory boundary validation
- Concurrent access patterns
- Production hardening features
- Comprehensive integration scenarios

### `stress_test_wallet_create.js`
Stress test specifically targeting the `wallet_create` function to validate segfault fixes under:
- High-load sequential operations
- Concurrent wallet creation
- Memory pressure scenarios
- Callback storms
- Worker thread interference

## Running Tests

### Prerequisites

1. **Build the wallet FFI library:**
   ```bash
   # From the project root
   cargo build --features nodejs_compatibility,debug_memory,debug_runtime
   ```

2. **Install Node.js dependencies (if using actual FFI):**
   ```bash
   npm install ffi-napi ref-napi
   ```

### Running the Tests

1. **Node.js Test Runner:**
   ```bash
   node integration_tests/nodejs/test_runner.js
   ```

2. **Stress Test:**
   ```bash
   node integration_tests/nodejs/stress_test_wallet_create.js
   ```

3. **Rust Integration Tests:**
   ```bash
   cargo test --manifest-path integration_tests/Cargo.toml --features nodejs_compatibility,debug_memory,debug_runtime
   ```

## Test Configuration

### Environment Variables
The tests recognize these Node.js environment variables:
- `NODE_VERSION` - Node.js version (automatically detected)
- `NODE_ENV` - Environment setting
- `npm_config_registry` - NPM registry URL

### Feature Flags
Tests utilize these Cargo feature flags:
- `nodejs_compatibility` - Enables Node.js-specific runtime strategies
- `debug_memory` - Enables memory diagnostics and boundary validation
- `debug_runtime` - Enables runtime conflict detection and logging

## Test Reports

All tests generate detailed JSON reports:
- `nodejs_ffi_test_report.json` - Comprehensive integration test results
- `wallet_create_stress_report.json` - Stress test performance metrics
- `runtime_conflict_report.json` - Runtime conflict analysis

## Understanding Test Results

### Success Criteria
- **Environment Detection**: Must correctly identify Node.js environment
- **Runtime Strategies**: At least one runtime strategy must initialize successfully
- **Memory Safety**: All FFI boundary validations must pass
- **Stress Testing**: Success rate should be ≥90% under load
- **Integration**: All subsystems must work together without conflicts

### Common Issues
- **FFI Library Not Found**: Ensure the wallet FFI has been built
- **Runtime Conflicts**: Indicates potential Tokio/Node.js event loop issues
- **Memory Violations**: Suggests FFI boundary safety problems
- **Worker Thread Failures**: May indicate threading compatibility issues

## Debugging Failed Tests

1. **Check Build**: Ensure the wallet FFI library is built with appropriate features
2. **Review Logs**: Examine detailed test output for specific failure points
3. **Memory Reports**: Check memory diagnostic reports for leaks or violations
4. **Runtime Analysis**: Review runtime strategy selection and conflicts

## Integration with CI/CD

These tests are designed to be run in continuous integration environments:

```bash
#!/bin/bash
# CI test script example

# Build with all features
cargo build --features nodejs_compatibility,debug_memory,debug_runtime

# Run Node.js integration tests
node integration_tests/nodejs/test_runner.js

# Run stress tests
node integration_tests/nodejs/stress_test_wallet_create.js

# Run Rust integration tests
cargo test --manifest-path integration_tests/Cargo.toml --features nodejs_compatibility,debug_memory,debug_runtime

echo "All Node.js integration tests completed"
```

## Contributing

When adding new tests:
1. Follow the existing pattern of comprehensive validation
2. Include both success and failure scenarios
3. Generate detailed reports for debugging
4. Document expected behavior and common issues
5. Test across different Node.js versions when possible

## Architecture

The test suite validates the comprehensive segfault fix implementation:

```
┌─────────────────────────────────────────────────────────────┐
│                    Node.js Application                      │
├─────────────────────────────────────────────────────────────┤
│                    FFI Boundary Layer                       │
│  ┌─────────────────┐ ┌─────────────────┐ ┌─────────────────┐ │
│  │ Memory Safety   │ │ Runtime         │ │ Production      │ │
│  │ Validation      │ │ Strategies      │ │ Hardening       │ │
│  └─────────────────┘ └─────────────────┘ └─────────────────┘ │
├─────────────────────────────────────────────────────────────┤
│              Tari Wallet FFI Library (Rust)                │
└─────────────────────────────────────────────────────────────┘
```

The tests validate each layer to ensure robust, crash-free operation in Node.js environments.
