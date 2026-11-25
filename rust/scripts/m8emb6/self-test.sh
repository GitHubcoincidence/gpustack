#!/usr/bin/env bash
# Offline refusal and identity tests for the M8-EMB.6 shell toolchain.
# shellcheck disable=SC1091
set -o errexit -o errtrace -o nounset -o pipefail

umask 077

readonly SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
source "$SCRIPT_DIR/common.sh"

readonly TEMP_ROOT="$(mktemp -d /tmp/m8emb6-self-test.XXXXXX)"
cleanup() {
  if [[ -d "$TEMP_ROOT" && ! -L "$TEMP_ROOT" ]]; then
    find "$TEMP_ROOT" -type f -exec unlink {} \; 2>/dev/null || true
    find "$TEMP_ROOT" -depth -type d -exec rmdir {} + 2>/dev/null || true
  fi
}
trap cleanup EXIT

printf '[1/6] bash syntax\n'
for script in "$SCRIPT_DIR"/*.sh; do
  bash -n "$script"
done

printf '[2/6] forbidden broad mutations\n'
if grep -REn -- '--privileged|docker (rm|stop)[[:space:]]+[$][(]|rm -rf|GPU-b1f89e65.*DeviceIDs' \
  "$SCRIPT_DIR/common.sh" \
  "$SCRIPT_DIR/prepare-model.sh" \
  "$SCRIPT_DIR/preflight.sh" \
  "$SCRIPT_DIR/executor.sh" \
  "$SCRIPT_DIR/cleanup.sh" \
  "$SCRIPT_DIR/watchdog.sh" \
  "$SCRIPT_DIR/post-audit.sh"; then
  printf 'STOP: forbidden broad mutation or GPU1 device request found\n' >&2
  exit 1
fi

printf '[3/6] process identity positive and negative paths\n'
sleep 30 &
test_pid=$!
m8emb6_record_process_identity "$test_pid" self_test "$TEMP_ROOT/process.txt"
m8emb6_process_identity_matches "$TEMP_ROOT/process.txt"
sed 's/^starttime=.*/starttime=1/' "$TEMP_ROOT/process.txt" >"$TEMP_ROOT/process-invalid.txt"
if m8emb6_process_identity_matches "$TEMP_ROOT/process-invalid.txt"; then
  printf 'STOP: invalid process identity was accepted\n' >&2
  exit 1
fi
kill -TERM "$test_pid"
wait "$test_pid" 2>/dev/null || true

install -m 0700 /bin/sleep "$TEMP_ROOT/deleted-sleep"
"$TEMP_ROOT/deleted-sleep" 30 &
deleted_pid=$!
unlink "$TEMP_ROOT/deleted-sleep"
m8emb6_record_process_identity "$deleted_pid" deleted_executable \
  "$TEMP_ROOT/deleted-process.txt"
m8emb6_process_identity_matches "$TEMP_ROOT/deleted-process.txt"
kill -TERM "$deleted_pid"
wait "$deleted_pid" 2>/dev/null || true

printf '[4/6] executor refuses missing maintenance authorization\n'
set +o errexit
M8EMB6_EXPECTED_HEAD='0000000000000000000000000000000000000000' \
M8EMB6_PREFLIGHT_EVIDENCE="$TEMP_ROOT/preflight" \
M8EMB6_PREFLIGHT_MANIFEST_SHA='0000000000000000000000000000000000000000000000000000000000000000' \
M8EMB6_EVIDENCE="$TEMP_ROOT/formal" \
M8EMB6_CLEANUP_EVIDENCE="$TEMP_ROOT/cleanup" \
M8EMB6_WINDOW_AUTHORIZATION="$TEMP_ROOT/missing-authorization.json" \
  "$SCRIPT_DIR/executor.sh" >"$TEMP_ROOT/refusal.stdout" 2>"$TEMP_ROOT/refusal.stderr"
refusal_exit=$?
set -o errexit
[[ "$refusal_exit" -ne 0 ]]
[[ ! -e "$TEMP_ROOT/formal" && ! -e "$TEMP_ROOT/cleanup" ]]
grep -q 'protected file is unavailable' "$TEMP_ROOT/refusal.stderr"

printf '[5/6] fixed model identity\n'
m8emb6_verify_model

printf '[6/6] evidence sealing preserves only the recovery executable mode\n'
install -d -m 0700 "$TEMP_ROOT/seal"
install -m 0700 /bin/true "$TEMP_ROOT/seal/existing-worker-binary"
printf 'evidence\n' >"$TEMP_ROOT/seal/payload.txt"
m8emb6_seal_directory "$TEMP_ROOT/seal"
[[ "$(stat -c '%a' "$TEMP_ROOT/seal/existing-worker-binary")" == '700' ]]
[[ "$(stat -c '%a' "$TEMP_ROOT/seal/payload.txt")" == '600' ]]
(cd "$TEMP_ROOT/seal" && sha256sum --check manifest.sha256) >/dev/null

install -d -m 0700 "$TEMP_ROOT/owned-tree/nested"
printf 'one\n' >"$TEMP_ROOT/owned-tree/one.txt"
printf 'two\n' >"$TEMP_ROOT/owned-tree/nested/two.txt"
ln -s one.txt "$TEMP_ROOT/owned-tree/one.link"
m8emb6_remove_owned_tree "$TEMP_ROOT/owned-tree" "$TEMP_ROOT/owned-tree"
[[ ! -e "$TEMP_ROOT/owned-tree" && ! -L "$TEMP_ROOT/owned-tree" ]]

printf 'm8emb6_toolchain_self_test=PASS\n'
