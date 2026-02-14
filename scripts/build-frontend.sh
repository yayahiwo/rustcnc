#!/usr/bin/env bash
set -euo pipefail

# Build the SolidJS frontend for embedding in the Rust binary
# Output: frontend/dist/

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
FRONTEND_DIR="$PROJECT_ROOT/frontend"

echo "==> Building frontend..."

cd "$FRONTEND_DIR"

# Install dependencies if needed
if [ ! -d "node_modules" ]; then
    echo "  Installing npm dependencies..."
    npm install
fi

# Build
echo "  Running Vite build..."
npx vite build

echo "==> Frontend built: $FRONTEND_DIR/dist/"
echo "    $(du -sh dist/ | cut -f1) total"
echo "    $(find dist/ -name '*.js' -o -name '*.css' | wc -l | tr -d ' ') output files"
