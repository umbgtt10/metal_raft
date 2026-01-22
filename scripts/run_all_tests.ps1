# Run all tests for raft-core and raft-sim

Write-Host "Running tests for raft-core..."
cargo nextest run -p raft-core

Write-Host "Running tests for raft-sim..."
cargo nextest run -p raft-sim

