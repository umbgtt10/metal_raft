Write-Host "Running Embassy Simulation with Channel Transport and In-Memory Storage..."
# We must disable default features because udp-transport is now the default
cargo run --release --no-default-features --features "channel-transport,in-memory-storage"
