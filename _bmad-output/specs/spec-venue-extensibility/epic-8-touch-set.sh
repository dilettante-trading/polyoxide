#!/usr/bin/env bash
cd "$(git rev-parse --show-toplevel)" || exit 2
{ git diff --name-only "$(git merge-base main HEAD)"; git ls-files --others --exclude-standard; } | sort -u | grep -v -E -f "$(dirname "$0")/epic-8-touch-set.txt"; test $? -eq 1
