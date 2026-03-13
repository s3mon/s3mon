test: clippy fmt
  cargo test

clippy:
  cargo clippy --all-targets --all-features

fmt:
  cargo fmt --all -- --check

coverage:
  cargo llvm-cov --all-features --workspace
