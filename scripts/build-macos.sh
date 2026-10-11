#!/bin/sh
# Builds the macOS executables on Linux, using zig as the linker (no Apple SDK needed):
#   target/universal2-apple-darwin/release/tally   Intel + Apple Silicon (ship this one)
#   target/{aarch64,x86_64}-apple-darwin/release/tally
# Needs: rustup targets aarch64-apple-darwin and x86_64-apple-darwin, plus `zig`, `cargo-zigbuild`
# and `llvm-lipo` on PATH. On a Mac, a plain `cargo build --release` is enough.
set -eu
cd "$(dirname "$0")/.."

# Minimum macOS versions: rustc's own defaults on a Mac (11.0 is the first for Apple Silicon).
# zig only takes the version from its target name and `cargo zigbuild` passes none (zig would then
# require 13.0), so link through a wrapper that names the versioned target. The wrapper still goes
# through `cargo-zigbuild zig cc`, which drops the Apple-only linker flags zig does not accept.
wrappers=target/zig-macos
mkdir -p "$wrappers"
for spec in aarch64:aarch64:11.0 x86_64:x86_64:10.12; do
    rust_arch=${spec%%:*}
    rest=${spec#*:}
    zig_arch=${rest%%:*}
    min=${rest#*:}
    cat > "$wrappers/$rust_arch-cc" <<EOF
#!/bin/sh
exec cargo-zigbuild zig cc -- -g -fno-sanitize=all -target $zig_arch-macos.$min-none "\$@"
EOF
    chmod +x "$wrappers/$rust_arch-cc"
done
export CARGO_TARGET_AARCH64_APPLE_DARWIN_LINKER="$PWD/$wrappers/aarch64-cc"
export CARGO_TARGET_X86_64_APPLE_DARWIN_LINKER="$PWD/$wrappers/x86_64-cc"

for target in aarch64-apple-darwin x86_64-apple-darwin; do
    cargo check --release --all-targets --target "$target"
    cargo build --release --target "$target" "$@"
done
mkdir -p target/universal2-apple-darwin/release
llvm-lipo -create -output target/universal2-apple-darwin/release/tally \
    target/aarch64-apple-darwin/release/tally target/x86_64-apple-darwin/release/tally
