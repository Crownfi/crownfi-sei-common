#!/bin/env sh

prettier -w package.json packages/npm
cargo fmt
