#!/bin/sh
set -eu

usage() {
    printf '%s\n' 'ERROR ARGUMENT: usage: check-layering.sh --root PATH [--self-test]' >&2
}

root=
self_test=0
while [ "$#" -gt 0 ]; do
    case "$1" in
        --root)
            if [ "$#" -lt 2 ] || [ -n "$root" ]; then
                usage
                exit 64
            fi
            root=$2
            shift 2
            ;;
        --self-test)
            if [ "$self_test" -ne 0 ]; then
                usage
                exit 64
            fi
            self_test=1
            shift
            ;;
        *)
            usage
            exit 64
            ;;
    esac
done

if [ -z "$root" ]; then
    usage
    exit 64
fi

case "$root" in
    /*) ;;
    *) root=$(pwd)/$root ;;
esac

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
project_manifest=$script_dir/../apps/Cargo.toml
mode=check
if [ "$self_test" -eq 1 ]; then
    mode=self-test
fi

if PIXBOT_ARCHITECTURE_GUARD_ROOT="$root" \
    PIXBOT_ARCHITECTURE_GUARD_MODE="$mode" \
    PIXBOT_ARCHITECTURE_GUARD_SCRIPT="$script_dir/check-layering.sh" \
    CARGO_NET_OFFLINE=true \
    cargo test --manifest-path "$project_manifest" \
    -p contracts --test architecture_guard -- \
    --exact architecture_guard_entrypoint --nocapture; then
    printf 'PASS: architecture guard (%s) for %s\n' "$mode" "$root"
else
    status=$?
    printf 'ERROR GUARD_COMMAND: architecture guard (%s) exited %s for %s\n' \
        "$mode" "$status" "$root" >&2
    exit "$status"
fi
