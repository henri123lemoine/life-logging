#!/bin/bash
DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
cd "$DIR"
cargo build --release
# Get the name of the binary from Cargo.toml
BINARY_NAME=$(grep -m1 'name =' Cargo.toml | cut -d '"' -f2)
# Run the program in the background
mkdir -p data/logs
nohup "./target/release/$BINARY_NAME" > data/logs/output.log 2>&1 &
echo "Program started in background. PID: $!"