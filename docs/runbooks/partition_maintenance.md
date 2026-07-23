# Partition Maintenance Runbook

## Scope

`audit.events` and `audit.login_attempts` are range-partitioned by month.
This runbook describes how to create future partitions, drop old partitions,
and run bounded batch cleanup using the built-in checkpoint table.

## Pre-create future partitions

Use the application maintenance job or the SQL function below to create the
next `N` monthly partitions. The migration already creates the current month
plus the next 12 months.

```sql
DO $$
DECLARE
    start_date DATE := date_trunc('month', now() + interval '1 year')::date;
    p_start DATE;
    p_end DATE;
    i INT;
BEGIN
    FOR i IN 1..12 LOOP
        p_start := start_date + (i || ' months')::interval;
        p_end := p_start + interval '1 month';
        EXECUTE format(
            'CREATE TABLE IF NOT EXISTS audit.%I PARTITION OF audit.events FOR VALUES FROM (%L) TO (%L)',
            'events_' || to_char(p_start, 'YYYY_MM'), p_start, p_end
        );
        EXECUTE format(
            'CREATE TABLE IF NOT EXISTS audit.%I PARTITION OF audit.login_attempts FOR VALUES FROM (%L) TO (%L)',
            'login_attempts_' || to_char(p_start, 'YYYY_MM'), p_start, p_end
        );
    END LOOP;
END
$$;
```

## Default partition policy

`audit.events_default` and `audit.login_attempts_default` only catch rows whose
`created_at` does not map to an existing monthly partition. This indicates a
missing partition or clock skew. Alert on any rows in these partitions, then
move or delete them after investigation.

## Bounded batch cleanup

`audit.purge_partition(table, partition, cutoff, batch_size)` deletes rows in
batches and updates `audit.cleanup_checkpoint`. The function is resumable: if a
run is interrupted, the next call continues from `last_id`.

Example: delete audit events older than 90 days from one partition:

```sql
SELECT audit.purge_partition(
    'events',
    'events_2026_01',
    now() - interval '90 days',
    1000
);
```

For whole partitions that are no longer needed, prefer `DROP TABLE` on the
partition rather than row-by-row deletion:

```sql
DROP TABLE audit.events_2025_01;
DROP TABLE audit.login_attempts_2025_01;
```

## Monitoring

Check the checkpoint table for in-progress or failed cleanups:

```sql
SELECT * FROM audit.cleanup_checkpoint ORDER BY updated_at DESC;
```

Verify no unexpected data is in default partitions:

```sql
SELECT count(*) FROM audit.events_default;
SELECT count(*) FROM audit.login_attempts_default;
```
