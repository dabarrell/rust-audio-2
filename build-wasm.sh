#!/bin/bash

# Set the necessary flags for atomics support
export RUSTFLAGS="-C target-feature=+atomics,+bulk-memory,+mutable-globals"

# Build the WASM module
cd crate && wasm-pack build --target web --out-dir ../public/wasm --no-pack
