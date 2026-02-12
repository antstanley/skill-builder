#!/bin/bash
# Tests for install.sh
# Run: bash tests/install_script_test.sh

set -eu

SCRIPT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
PASS=0
FAIL=0

# Source the install script in test mode
# shellcheck source=../install.sh
. "$SCRIPT_DIR/install.sh" --test-mode

assert_eq() {
    local test_name="$1"
    local expected="$2"
    local actual="$3"

    if [ "$expected" = "$actual" ]; then
        echo "  PASS: $test_name"
        PASS=$((PASS + 1))
    else
        echo "  FAIL: $test_name (expected '$expected', got '$actual')"
        FAIL=$((FAIL + 1))
    fi
}

assert_success() {
    local test_name="$1"
    shift

    if "$@" >/dev/null 2>&1; then
        echo "  PASS: $test_name"
        PASS=$((PASS + 1))
    else
        echo "  FAIL: $test_name (command failed: $*)"
        FAIL=$((FAIL + 1))
    fi
}

assert_failure() {
    local test_name="$1"
    shift

    if "$@" >/dev/null 2>&1; then
        echo "  FAIL: $test_name (expected failure but succeeded: $*)"
        FAIL=$((FAIL + 1))
    else
        echo "  PASS: $test_name"
        PASS=$((PASS + 1))
    fi
}

echo "=== install.sh tests ==="
echo ""

# --- detect_os tests ---
echo "detect_os:"
os_result=$(detect_os)
case "$os_result" in
    linux|macos|windows)
        echo "  PASS: detect_os returns valid value ($os_result)"
        PASS=$((PASS + 1))
        ;;
    *)
        echo "  FAIL: detect_os returned unexpected value: $os_result"
        FAIL=$((FAIL + 1))
        ;;
esac

# --- detect_arch tests ---
echo ""
echo "detect_arch:"
arch_result=$(detect_arch)
case "$arch_result" in
    x86_64|aarch64)
        echo "  PASS: detect_arch returns valid value ($arch_result)"
        PASS=$((PASS + 1))
        ;;
    *)
        echo "  FAIL: detect_arch returned unexpected value: $arch_result"
        FAIL=$((FAIL + 1))
        ;;
esac

# --- get_target tests ---
echo ""
echo "get_target:"
assert_eq "linux x86_64"    "x86_64-linux-gnu"   "$(get_target linux x86_64)"
assert_eq "linux aarch64"   "aarch64-linux-gnu"  "$(get_target linux aarch64)"
assert_eq "macos x86_64"    "x86_64-apple-darwin"        "$(get_target macos x86_64)"
assert_eq "macos aarch64"   "aarch64-apple-darwin"       "$(get_target macos aarch64)"
assert_eq "windows x86_64"  "x86_64-pc-windows-msvc"     "$(get_target windows x86_64)"
assert_failure "unsupported platform fails" get_target linux riscv64

# --- get_default_install_dir tests ---
echo ""
echo "get_default_install_dir:"
assert_eq "linux default dir"   "$HOME/.local/bin"  "$(get_default_install_dir linux)"
assert_eq "macos default dir"   "/usr/local/bin"    "$(get_default_install_dir macos)"
assert_eq "windows default dir" "$HOME/.local/bin"  "$(get_default_install_dir windows)"

# --- get_archive_ext tests ---
echo ""
echo "get_archive_ext:"
assert_eq "linux ext"   "tar.gz" "$(get_archive_ext linux)"
assert_eq "macos ext"   "tar.gz" "$(get_archive_ext macos)"
assert_eq "windows ext" "zip"    "$(get_archive_ext windows)"

# --- Summary ---
echo ""
echo "=== Results ==="
TOTAL=$((PASS + FAIL))
echo "  $PASS/$TOTAL passed"

if [ "$FAIL" -gt 0 ]; then
    echo "  $FAIL FAILED"
    exit 1
fi

echo "  All tests passed!"
