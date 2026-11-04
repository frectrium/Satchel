#!/usr/bin/env bash
# Runs the integration tests (crates/satchel-it) against a local Compose
# cluster: builds the image once, starts the cluster, runs the tests, prints
# the cluster logs if anything fails and always tears the cluster down.
#
#   scripts/integration-test.sh
#
# SATCHEL_IT_SKIP_BUILD=1 skips the image build, for CI, which builds
# satchel:dev itself with a layer cache.
set -euo pipefail

cd "$(dirname "$0")/.."

cleanup() {
    echo "--- Tearing down the cluster"
    docker compose down -v --remove-orphans
}
trap cleanup EXIT

show_logs() {
    echo "--- Cluster logs" >&2
    docker compose logs --no-color >&2
}

if [[ "${SATCHEL_IT_SKIP_BUILD:-0}" != "1" ]]; then
    # Build once. Every service shares satchel:dev, and `docker compose up
    # --build` would build it once per service in parallel, which can fail.
    echo "--- Building satchel:dev"
    docker build -t satchel:dev .
fi

echo "--- Starting the cluster"
if ! docker compose up -d --wait --wait-timeout 120; then
    echo "The cluster did not become healthy." >&2
    show_logs
    exit 1
fi

echo "--- Running the integration tests"
if ! cargo test -p satchel-it --locked -- --ignored; then
    echo "Integration tests failed." >&2
    show_logs
    exit 1
fi
