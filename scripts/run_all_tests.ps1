# Run all tests for raft-core and raft-validation

Write-Host "Running tests for raft-core..."
cargo nextest run -p raft-core

Write-Host "Running tests for raft-validation..."
cargo nextest run -p raft-validation

