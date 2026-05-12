#!/usr/bin/env bash

set -exEuo pipefail

# Output directories for the profile files and coverage
#
# You'll find the coverage report and `lcov` file under: $CARGO_TARGET_DIR/debug/coverage/
#
# Use absolute paths every where
CARGO_TARGET_DIR=$(
    cargo metadata --format-version 1 --no-deps --locked |
        jq '.target_directory' --raw-output
)
export CARGO_TARGET_DIR
export CARGO_INCREMENTAL=0

gethtml_wrapper() {

    # Better branch coverage information
    if ! command -v genhtml; then
        echo 'Command genhtml not found'
        return
    fi

    mkdir -p "$COVERAGE_OUT_DIR/genhtml"
    genhtml \
        --show-details \
        --legend \
        --branch-coverage \
        --dark-mode \
        --missed \
        --demangle-cpp rustfilt \
        --output-directory "$COVERAGE_OUT_DIR/$1/genhtml" \
        "$COVERAGE_OUT_DIR/$1/lcov.info"

}

crate="phoenix-chan"

if [[ -n "${EXPORT_FOR_CI:-}" ]]; then
    out_path="$PWD/coverage-$crate.info"
else
    mkdir -p "$CARGO_TARGET_DIR/lcov"
    out_path="$CARGO_TARGET_DIR/lcov/coverage-$crate.info"
fi

# Currently branch coverage can be broken on nightly
cargo +nightly llvm-cov \
    --all-features -p "$crate" \
    --lcov \
    --output-path "$out_path"

cargo llvm-cov report

if [[ -z "${EXPORT_FOR_CI:-}" ]]; then
    cargo llvm-cov report
    cargo llvm-cov report --html
else
    {
        echo '# Code Coverage'
        echo ''
        echo '```'
        cargo llvm-cov report
        echo '```'
    } >>"$GITHUB_STEP_SUMMARY"
fi

if [[ -n "${EXPORT_BASE_COMMIT:-}" ]]; then
    commit=$(git rev-parse HEAD)
    cp -v "$out_path" "$COVERAGE_OUT_DIR/baseline-$commit.info"
    echo "$commit" >"$COVERAGE_OUT_DIR/baseline-commit.txt"
fi
