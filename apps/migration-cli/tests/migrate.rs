use sqlx::{Pool, Postgres};

#[sqlx::test(migrations = "../../migrations")]
async fn schemas_and_roles_are_created(pool: Pool<Postgres>) -> sqlx::Result<()> {
    let row: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM information_schema.schemata WHERE schema_name = ANY($1)",
    )
    .bind(
        &[
            "iam", "org", "authz", "resource", "audit", "config", "infra",
        ][..],
    )
    .fetch_one(&pool)
    .await?;
    assert_eq!(row.0, 7);

    let role: (bool,) =
        sqlx::query_as("SELECT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'clsc_app')")
            .fetch_one(&pool)
            .await?;
    assert!(role.0);
    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn common_authoritative_table_pattern_exists(pool: Pool<Postgres>) -> sqlx::Result<()> {
    let row: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM information_schema.columns
         WHERE table_schema = 'iam' AND table_name = 'tenants'",
    )
    .fetch_one(&pool)
    .await?;
    assert!(row.0 >= 9);

    let check: (bool,) = sqlx::query_as(
        "SELECT EXISTS (
            SELECT 1 FROM information_schema.table_constraints
            WHERE table_schema = 'iam' AND table_name = 'tenants' AND constraint_type = 'CHECK'
        )",
    )
    .fetch_one(&pool)
    .await?;
    assert!(check.0);
    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn schema_metadata_table_exists(pool: Pool<Postgres>) -> sqlx::Result<()> {
    let row: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM information_schema.tables
         WHERE table_schema = 'infra' AND table_name = 'schema_metadata'",
    )
    .fetch_one(&pool)
    .await?;
    assert_eq!(row.0, 1);
    Ok(())
}
