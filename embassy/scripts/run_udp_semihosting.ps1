Write-Host "Running Embassy Simulation with UDP Transport and Semihosting Storage..."
# Explicitly specify both udp-transport and semihosting-storage features
cargo run --release --no-default-features --features "udp-transport,semihosting-storage"
