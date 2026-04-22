#!/bin/bash
# Hook wrapper: pipe stdin to ivara capture.
# Exits 0 always — logging must never block the agent.

command -v ivara >/dev/null 2>&1 || exit 0

INPUT=$(cat)
echo "$INPUT" | ivara capture 2>/dev/null

exit 0
