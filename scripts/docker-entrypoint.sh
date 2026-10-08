#!/bin/sh
set -eu

# Render supplies PORT. An explicit BIND_ADDR still takes precedence.
export BIND_ADDR="${BIND_ADDR:-0.0.0.0:${PORT:-10000}}"

# Let the application receive SIGTERM directly for graceful shutdown.
exec "$@"
