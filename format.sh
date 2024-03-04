#!/bin/env sh

npx prettier -w package.json packages/npm
cargo fmt
