#!/bin/bash

set -uo pipefail

cd "$(dirname "$0")/.." || exit 1

passed=0
failed=0
WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT

pass() { passed=$((passed + 1)); }
fail() {
  printf '    %s\n' "$1" >&2
  failed=$((failed + 1))
}

holds() {
  local why=$1
  shift
  if "$@"; then
    pass
  else
    fail "$why"
  fi
}

refuses() {
  local why=$1
  shift
  if "$@"; then
    fail "$why"
  else
    pass
  fi
}

check() {
  local name=$1 __failures_before=$failed status
  "$name"
  status=$?
  if ((status != 0 && failed == __failures_before)); then
    fail "the test itself exited with $status"
  fi
  if ((failed == __failures_before)); then
    printf 'ok - %s\n' "${name//_/ }"
  else
    printf 'not ok - %s\n' "${name//_/ }"
  fi
}

declared_gates() {
  sed -nE 's/^GATES=\((.*)\)$/\1/p' scripts/pre-commit | tr ' ' '\n'
}

implemented_gates() {
  grep -oE '^if wanted [a-z-]+' scripts/pre-commit | awk '{print $3}'
}

gates_ci_runs() {
  grep -oE '\./scripts/pre-commit [a-z-]+' .github/workflows/ci.yml | awk '{print $2}' | sort -u
}

the_declared_gates_are_the_implemented_ones() {
  local declared implemented
  declared=$(declared_gates | sort)
  implemented=$(implemented_gates | sort)
  holds "GATES lists [$declared] but the script runs [$implemented]" test "$declared" = "$implemented"
}

is_gate() {
  declared_gates | grep -qx -- "$1"
}

runs_in_ci() {
  gates_ci_runs | grep -qx -- "$1"
}

every_gate_ci_names_exists() {
  local gate
  for gate in $(gates_ci_runs); do
    holds "ci.yml runs '$gate', which is not a gate" is_gate "$gate"
  done
}

every_gate_runs_in_ci() {
  local gate
  for gate in $(declared_gates); do
    holds "gate '$gate' never runs in ci.yml" runs_in_ci "$gate"
  done
}

an_unknown_gate_is_refused() {
  local output status
  output=$(./scripts/pre-commit no-such-gate 2>&1)
  status=$?
  holds "an unknown gate exited $status" test "$status" -eq 1
  holds "an unknown gate said: $output" test "${output%%$'\n'*}" = "no such gate: no-such-gate"
}

warnings_fail_every_build() {
  holds "pre-commit does not make compiler warnings fatal" grep -qx 'export RUSTFLAGS="-D warnings"' scripts/pre-commit
  holds "ci.yml does not make compiler warnings fatal" grep -qx '  RUSTFLAGS: -D warnings' .github/workflows/ci.yml
  holds "clippy warnings are not fatal" grep -q 'cargo clippy .* -- -D warnings' scripts/pre-commit
  holds "cargo deny warnings are not fatal" grep -q 'cargo deny .*--deny warnings' scripts/pre-commit
}

message() {
  printf '%s\n' "$@" >"$WORK/msg"
  ./scripts/commit-msg "$WORK/msg" >/dev/null 2>&1
}

a_conventional_message_is_accepted() {
  holds "a conventional message was refused" message "feat: add the json reporter"
  holds "a scoped breaking message was refused" message "fix(sandbox)!: keep symlinks as links" "" "Body text."
}

a_message_that_is_not_conventional_is_refused() {
  refuses "a message with no type was accepted" message "Add the json reporter"
  refuses "an uppercase description was accepted" message "feat: Add the json reporter"
}

check the_declared_gates_are_the_implemented_ones
check every_gate_ci_names_exists
check every_gate_runs_in_ci
check an_unknown_gate_is_refused
check warnings_fail_every_build
check a_conventional_message_is_accepted
check a_message_that_is_not_conventional_is_refused

printf '\n%d passed, %d failed\n' "$passed" "$failed"
((failed == 0))
