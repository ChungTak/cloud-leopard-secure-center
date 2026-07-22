#!/usr/bin/env bash
set -euo pipefail

# Verify that pg_dump and pg_restore can round-trip the CLSC database.
# Usage: DATABASE_URL=postgres://postgres:postgres@localhost:5432/clsc ./scripts/verify_pg_dump_restore.sh

DATABASE_URL="${DATABASE_URL:-postgres://postgres:postgres@localhost:5432/clsc}"
ADMIN_URL="${DATABASE_URL%/*}/postgres"
TEMP_DB="clsc_restore_test_$$"
DUMP_FILE="clsc_dump_$$.sql"

cleanup() {
    rm -f "${DUMP_FILE}"
    psql "${ADMIN_URL}" -c "DROP DATABASE IF EXISTS ${TEMP_DB};" >/dev/null 2>&1 || true
}
trap cleanup EXIT

echo "Dumping source database..."
pg_dump --no-owner --no-privileges --clean --if-exists \
    "${DATABASE_URL}" >"${DUMP_FILE}"

echo "Creating temporary restore database ${TEMP_DB}..."
psql "${ADMIN_URL}" -c "CREATE DATABASE ${TEMP_DB};" >/dev/null

echo "Restoring into temporary database..."
RESTORE_URL="${DATABASE_URL%/*}/${TEMP_DB}"
psql "${RESTORE_URL}" <"${DUMP_FILE}" >/dev/null

echo "Comparing row counts..."
SOURCE_COUNTS=$(psql "${DATABASE_URL}" -At -c "
    SELECT n.nspname || '.' || c.relname || '=' || count(*)::text
    FROM pg_class c
    JOIN pg_namespace n ON n.oid = c.relnamespace
    WHERE c.relkind IN ('r','p')
      AND n.nspname IN ('iam','audit','org','authz','resource','config','infra','app')
    GROUP BY n.nspname, c.relname
    ORDER BY 1;
")

RESTORE_COUNTS=$(psql "${RESTORE_URL}" -At -c "
    SELECT n.nspname || '.' || c.relname || '=' || count(*)::text
    FROM pg_class c
    JOIN pg_namespace n ON n.oid = c.relnamespace
    WHERE c.relkind IN ('r','p')
      AND n.nspname IN ('iam','audit','org','authz','resource','config','infra','app')
    GROUP BY n.nspname, c.relname
    ORDER BY 1;
")

if [ "${SOURCE_COUNTS}" != "${RESTORE_COUNTS}" ]; then
    echo "Row counts differ between source and restore:"
    diff <(echo "${SOURCE_COUNTS}") <(echo "${RESTORE_COUNTS}") || true
    exit 1
fi

echo "pg_dump/restore verification passed."
