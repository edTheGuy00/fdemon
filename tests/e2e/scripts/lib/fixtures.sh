#!/bin/bash
# Fixture helper functions for E2E testing

# Ensure Linux platform is properly set up for a fixture
# macOS-generated linux/ dirs are stubs and need regeneration in Docker
setup_fixture_linux() {
    local fixture_dir=$1

    if [[ ! -d "$fixture_dir" ]]; then
        echo "ERROR: Fixture directory not found: $fixture_dir"
        return 1
    fi

    local orig_dir=$(pwd)
    cd "$fixture_dir"

    # Check if Linux platform needs regeneration (stub CMakeLists.txt is < 100 bytes)
    local needs_regen=false
    if [[ -f "linux/CMakeLists.txt" ]]; then
        local size=$(stat -c%s "linux/CMakeLists.txt" 2>/dev/null || stat -f%z "linux/CMakeLists.txt" 2>/dev/null || echo "0")
        if [[ "$size" -lt 100 ]]; then
            needs_regen=true
        fi
    else
        needs_regen=true
    fi

    if [[ "$needs_regen" == "true" ]]; then
        echo "Regenerating Linux platform for $(basename $fixture_dir)..."
        rm -rf linux/
        flutter create --platforms=linux . > /dev/null 2>&1
        if [[ $? -ne 0 ]]; then
            echo "ERROR: Failed to create Linux platform for $fixture_dir"
            cd "$orig_dir"
            return 1
        fi
    fi

    cd "$orig_dir"
    return 0
}
