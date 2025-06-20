#!/bin/sh

generate_docs() {
    dir="$1"
    cd "$dir" && cargo +nightly rustdoc --lib -- -Z unstable-options --output-format json && cd ..
}

generate_docs pete/
generate_docs psyche/
generate_docs lingproc/