#!/bin/sh

echo "Locating home directory..."
home_dir=$(eval echo ~$USER)
echo "Using home directory: $home_dir"

bin_dir="$home_dir/.local/bin"

echo "Checking for bin directory..."
if [ ! -d "$bin_dir" ]; then
    echo "Creating bin directory: $bin_dir"
    mkdir -p "$bin_dir"
fi

echo "Checking if bin directory is in PATH..."
if ! echo "$PATH" | grep -Eq "(^|:)$bin_dir($|:)"; then
    rcfile="$home_dir/.bashrc"

    echo "Checking for .bashrc file..."
    if [ ! -f "$rcfile" ]; then
        rcfile="$home_dir/.zshrc"
        if [ ! -f "$rcfile" ]; then
            echo "Error: Neither .bashrc nor .zshrc found"
            exit 1
        else
            echo "Using .zshrc file: $rcfile"
        fi
    else
        echo "Using .bashrc file: $rcfile"
    fi

    echo "Adding bin directory to \$PATH in $rcfile"
    echo "export PATH=\"\$PATH:$bin_dir\"" >> "$rcfile"
fi

echo "Building release version..."
if cargo build --release &> /dev/null; then
    echo "Copying binary to bin directory..."
    cp target/release/prodomme "$bin_dir/prodomme"
else
    echo "Error: Failed to build release version"
fi

