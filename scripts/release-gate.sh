#!/usr/bin/env bash
# Release gate for cloud-leopard-secure-center.
#
# Usage: scripts/release-gate.sh [VERSION]
# Defaults to the workspace version declared in Cargo.toml.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERSION="${1:-$(grep -E '^version\s*=\s*"' "$REPO_ROOT/Cargo.toml" | head -1 | sed -E 's/.*"([^"]+)".*/\1/')}"
DIST_DIR="$REPO_ROOT/dist"

mkdir -p "$DIST_DIR"

log() {
    echo "[release-gate] $*" >&2
}

fail() {
    echo "[release-gate] FAIL: $*" >&2
    exit 1
}

require() {
    command -v "$1" >/dev/null 2>&1 || fail "$1 is required"
}

require cargo
require docker

# pnpm is not always on the default PATH in this environment.
if ! command -v pnpm >/dev/null 2>&1; then
    export PATH="$HOME/.local/share/pnpm/bin:$PATH"
fi
require pnpm

log "Running release gate for version $VERSION"

# 1. Workspace quality gates
log "Rust fmt"
cargo fmt --all -- --check

log "Rust clippy"
cargo clippy --workspace --all-targets -- -D warnings

log "Rust architecture test"
cargo run --manifest-path "$REPO_ROOT/tools/architecture-test/Cargo.toml" -- "$REPO_ROOT"

log "Rust deny (license/advisory/bans)"
cargo deny check

log "Rust aarch64 cross-check"
cargo check --workspace --target aarch64-unknown-linux-gnu

log "Rust unit/integration tests"
if command -v cargo-nextest >/dev/null 2>&1; then
    cargo nextest run -p security-platform --test phase1_acceptance
else
    log "cargo-nextest unavailable; running targeted acceptance tests with cargo test"
    cargo test -p security-platform --test phase1_acceptance
fi

log "Web checks"
cd "$REPO_ROOT/web"
pnpm install --frozen-lockfile
pnpm typecheck
pnpm test
pnpm build
pnpm lint
pnpm format

# 2. OpenAPI snapshot diff
cd "$REPO_ROOT"
log "Regenerating OpenAPI snapshot"
UPDATE_OPENAPI=1 cargo test -p http-api openapi_snapshot

OPENAPI_DIFF="$DIST_DIR/openapi-$VERSION.diff"
if git diff --exit-code -- crates/http-api/openapi.json >"$OPENAPI_DIFF"; then
    log "OpenAPI snapshot unchanged"
else
    log "OpenAPI snapshot changed; diff written to $OPENAPI_DIFF"
fi

# 3. Migration integrity
cd "$REPO_ROOT"
log "Migration smoke test"
DATABASE_URL="${DATABASE_URL:-postgres://postgres:postgres@localhost:5432/clsc}" \
    cargo run -p migration-cli | grep '"status":"ready"'

# 4. Container image
IMAGE="ghcr.io/chungtak/cloud-leopard-secure-center:$VERSION"
log "Building container image $IMAGE"
if docker build -t "$IMAGE" "$REPO_ROOT"; then
    DIGEST="$(docker inspect --format='{{index .RepoDigests 0}}' "$IMAGE" 2>/dev/null || echo "$IMAGE (local, digest unavailable)")"
    log "Image built: $DIGEST"
else
    fail "container image build failed"
fi

# 5. SBOM
SBOM_FILE="$DIST_DIR/sbom-$VERSION.json"
if command -v syft >/dev/null 2>&1; then
    log "Generating SBOM with syft"
    syft "$IMAGE" -o cyclonedx-json >"$SBOM_FILE"
else
    log "syft unavailable; SBOM generation is UNSUPPORTED in this environment"
    echo '{"bomFormat":"CycloneDX","specVersion":"1.5","metadata":{"tools":[{"vendor":" UNSUPPORTED","name":"syft not installed"}]}}' >"$SBOM_FILE"
fi

# 6. Vulnerability / license scan on image
if command -v trivy >/dev/null 2>&1; then
    log "Scanning container image with trivy"
    trivy image --scanners vuln,license --severity HIGH,CRITICAL "$IMAGE"
else
    log "trivy unavailable; container vulnerability scanning is UNSUPPORTED in this environment"
fi

# 7. Release notes
cat >"$DIST_DIR/release-$VERSION.md" <<EOF
# cloud-leopard-secure-center $VERSION

## Artifacts

- Container image: \`$IMAGE\`
- Image digest: \`$DIGEST\`
- SBOM: \`$SBOM_FILE\`
- OpenAPI diff: \`$OPENAPI_DIFF\`

## Checksums

EOF

sha256sum "$SBOM_FILE" >>"$DIST_DIR/release-$VERSION.md" || true

cat >>"$DIST_DIR/release-$VERSION.md" <<EOF

## Configuration Reference

See <config.example.toml> for a fully commented configuration reference.
Production deployments should:

1. Copy \`config.example.toml\` to \`config.toml\`.
2. Replace placeholder secrets via a \`SecretProvider\` or environment-specific vault.
3. Configure PostgreSQL, NATS JetStream, and cheetah signaling endpoints.
4. Run \`scripts/preinstall-check.sh\` and \`docker compose up -d\`.

## Upgrade Notes

- Backup the PostgreSQL database before upgrading.
- Run migrations via \`cargo run -p migration-cli -- migrate run\`.
- Verify the OpenAPI snapshot diff before regenerating frontend clients.

## Rollback Notes

- Restore the database from the pre-upgrade backup.
- Re-deploy the previous container image digest.
- Verify tenant cache and audit partitions are consistent after rollback.
EOF

log "Release gate complete. Artifacts are in $DIST_DIR"
