#!/bin/sh
# Fail when a shared library imports a glibc symbol version newer than the
# given one (ADR 0039).
#
#   tools/check_glibc.sh LIBRARY 2.28
set -eu
library=$1
maximum=GLIBC_$2
newest=$(objdump -T "$library" | grep -o 'GLIBC_[0-9.]*' | sort -uV | tail -n 1)
highest=$(printf '%s\n%s\n' "$newest" "$maximum" | sort -V | tail -n 1)
if [ "$highest" != "$maximum" ]; then
  echo "$library requires $newest, newer than $maximum" >&2
  exit 1
fi
echo "$library requires at most $newest"
