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

release_repo() {
  local repo=$WORK/release
  rm -rf "$repo"
  mkdir -p "$repo/scripts" "$WORK/bin"
  cp scripts/pre-commit "$repo/scripts/"
  printf '[package]\nversion = "1.0.0"\n' >"$repo/Cargo.toml"
  git -C "$repo" init -q
  git -C "$repo" add .
  git -C "$repo" -c user.name=gate -c user.email=gate@example.com commit -qm "feat: start"
  if [[ -n ${1:-} ]]; then
    git -C "$repo" tag "$1"
  fi
}

git_cliff_says() {
  printf '#!/bin/bash\n%s\n' "$1" >"$WORK/bin/git-cliff"
  chmod +x "$WORK/bin/git-cliff"
}

release_gate_passes() {
  (cd "$WORK/release" && PATH="$WORK/bin:$PATH" QMUTANT_BRANCH=release/1.0.0 ./scripts/pre-commit version >/dev/null 2>&1)
}

a_release_whose_required_version_cannot_be_read_is_refused() {
  release_repo v0.9.0
  git_cliff_says 'exit 1'
  refuses "a release passed although git-cliff could not say what the commits require" release_gate_passes
}

a_release_below_what_the_commits_require_is_refused() {
  release_repo v0.9.0
  git_cliff_says 'echo v2.0.0'
  refuses "1.0.0 passed although the commits require 2.0.0" release_gate_passes
}

a_release_meeting_what_the_commits_require_passes() {
  release_repo v0.9.0
  git_cliff_says 'echo v1.0.0'
  holds "1.0.0 was refused although the commits require exactly 1.0.0" release_gate_passes
}

the_first_release_has_no_previous_version_to_bump() {
  release_repo
  git_cliff_says 'exit 1'
  holds "the first release was refused for lacking a previous tag" release_gate_passes
}

check the_declared_gates_are_the_implemented_ones
check every_gate_ci_names_exists
check every_gate_runs_in_ci
check an_unknown_gate_is_refused
check warnings_fail_every_build
check a_conventional_message_is_accepted
check a_message_that_is_not_conventional_is_refused
check a_release_whose_required_version_cannot_be_read_is_refused
check a_release_below_what_the_commits_require_is_refused
check a_release_meeting_what_the_commits_require_passes
check the_first_release_has_no_previous_version_to_bump

printf '\n%d passed, %d failed\n' "$passed" "$failed"
((failed == 0))
