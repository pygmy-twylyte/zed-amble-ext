#!/bin/bash

# Build script for Amble LSP Extension
# This rebuilds the language server and prepares it for Zed

set -e  # Exit on error

echo "🔨 Building Amble Language Server..."
echo

# Navigate to language server directory
cd "$(dirname "$0")/language-server"

# Build the language server in release mode
echo "Building language server binary..."
cargo build --release

# Go back to extension root
cd ..

# Create bin directory if it doesn't exist
mkdir -p bin

# Copy the binary to bin directory
echo "Copying binary to bin/..."
cp language-server/target/release/amble-lsp bin/

# Make sure it's executable
chmod +x bin/amble-lsp

# Verify the binary
if [ -f bin/amble-lsp ]; then
    echo "✅ Binary successfully built and copied to bin/"
    echo "   Size: $(du -h bin/amble-lsp | cut -f1)"
else
    echo "❌ Error: Binary not found in bin/"
    exit 1
fi

echo
echo "🎉 Build complete!"
echo
echo "Next steps:"
echo "1. Uninstall old extension in Zed (if installed)"
echo "2. Quit Zed completely"
echo "3. Run: zed --dev-extension $(pwd)"
echo "4. Open a .amble file and test with F12"
