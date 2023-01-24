#!/bin/bash

cargo build || exit

trap 'kill $(jobs -p) 2>/dev/null' EXIT

PORT=12345
cargo run --bin=concurrent_git_pool -- --port "$PORT" 1>out.txt 2>&1 &

CONCURRENT_GIT_POOL="127.0.0.1:$PORT" cargo test --all-features
