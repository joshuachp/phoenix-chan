#!/usr/bin/env bash

set -exEuo pipefail

# Check if the crate can be compiled with only the files that will be packaged when publishing

# List files in a package
listPackage() {
    cargo package --allow-dirty -l -p "$1" | xargs -I '{}' echo "$1/{}"
}

pkgsFiles=$(
    # Add <(listPackage "@OTHER_CRATE@") \
    cat \
        <(cargo package --allow-dirty -l -p "@MAIN_CRATE@") |
        sort
)
localFiles=$(
    git ls-files -cdmo | sort -u
)

# List files unique to localFiles and not present in pkgsFiles
toCopy=$(comm -12 <(echo "$localFiles") <(echo "$pkgsFiles"))

workingDir="$(mktemp -d)"

mkdir -p "$workingDir"

cp -v Cargo.toml Cargo.lock "$workingDir"

echo "$toCopy" | while read -r file; do
    parent=$(dirname "$file")
    mkdir -p "$workingDir/${parent}"

    cp -v "$file" "$workingDir/$file"
done

cargo publish --dry-run --manifest-path "$workingDir/Cargo.toml" --workspace --all-features --locked
