#!/bin/sh

set -eu

echo "cleaning build directory:"
cargo clean

echo "building:"
cargo build

echo "done!"
