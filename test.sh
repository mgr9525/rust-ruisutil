

export RUST_BACKTRACE=1
cargo test --lib --package ruisutil --features all --no-default-features -- "tests::$1" --exact --nocapture

