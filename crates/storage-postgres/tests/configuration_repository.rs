use domain_configuration::{ConfigDefinition, ConfigScope, ConfigValue, ConfigValueType};
use domain_organization::tenant::Tenant;
use foundation::{
    FakeClock, RequestContext, Revision, SystemClock, SystemRandom, TenantId, uuid::Uuid,
};
use storage_api::{ConfigurationRepository, TenantRepository};
use storage_postgres::configuration_repository::PostgresConfigurationRepository;
use storage_postgres::tenant_repository::PostgresTenantRepository;

fn ok_or_panic<T, E: std::fmt::Debug>(result: Result<T, E>) -> T {
    match result {
        Ok(v) => v,
        Err(e) => panic!("{e:?}"),
    }
}

fn some_or_panic<T>(opt: Option<T>) -> T {
    match opt {
        Some(v) => v,
        None => panic!("expected Some value"),
    }
}

fn tenant_ctx(tenant: &str) -> RequestContext {
    RequestContext {
        tenant_id: Some(TenantId::parse_str(tenant).unwrap_or_else(|e| panic!("{e}"))),
        ..Default::default()
    }
}

fn any_ctx() -> RequestContext {
    RequestContext {
        tenant_id: Some(
            TenantId::parse_str("018e1234-5678-7abc-8def-0123456789ab")
                .unwrap_or_else(|e| panic!("{e}")),
        ),
        ..Default::default()
    }
}

async fn create_tenant(pool: &sqlx::PgPool, id: Uuid, code: &str, name: &str) -> TenantId {
    let repo = PostgresTenantRepository::new(pool.clone());
    let tenant = ok_or_panic(Tenant::new(
        TenantId::parse_str(&id.to_string()).unwrap_or_else(|e| panic!("{e}")),
        code,
        name,
        Option::<&str>::None,
        Option::<&str>::None,
        &FakeClock::from_millis(1_000_000_000_000),
        None,
    ));
    ok_or_panic(repo.create(&tenant, &tenant_ctx(&id.to_string())).await);
    TenantId::parse_str(&id.to_string()).unwrap_or_else(|e| panic!("{e}"))
}

fn int_definition(key: &str, default: &str) -> ConfigDefinition {
    ok_or_panic(ConfigDefinition::new(
        key,
        ConfigValueType::Integer,
        None,
        default,
        false,
        false,
    ))
}

fn secret_definition(key: &str) -> ConfigDefinition {
    ok_or_panic(ConfigDefinition::new(
        key,
        ConfigValueType::Secret,
        None,
        "${secret:default}",
        true,
        false,
    ))
}

#[sqlx::test(migrations = "../../migrations")]
async fn definition_and_value_round_trip(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let repo = PostgresConfigurationRepository::new(pool.clone(), SystemClock, SystemRandom);
    let definition = int_definition("http.port", "8080");
    ok_or_panic(repo.save_definition(&definition).await);

    let value = ok_or_panic(ConfigValue::new(
        None,
        ConfigScope::Platform,
        &definition,
        "3000",
        None,
        Revision::new(0),
    ));

    let saved = ok_or_panic(repo.save_value(&value, &any_ctx()).await);
    assert!(saved.id.is_some());

    let fetched = some_or_panic(ok_or_panic(
        repo.get_value("http.port", &ConfigScope::Platform, &any_ctx())
            .await,
    ));
    assert_eq!(fetched.raw_value, "3000");

    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn scope_precedence_module_tenant_platform(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let tenant_id = create_tenant(
        &pool,
        Uuid::parse_str("018e1234-5678-7abc-8def-0123456789ab").unwrap_or_else(|e| panic!("{e}")),
        "t1",
        "Tenant",
    )
    .await;
    let ctx = tenant_ctx(&tenant_id.as_uuid().to_string());
    let repo = PostgresConfigurationRepository::new(pool.clone(), SystemClock, SystemRandom);
    let definition = int_definition("http.port", "8080");
    ok_or_panic(repo.save_definition(&definition).await);

    let platform = ok_or_panic(ConfigValue::new(
        None,
        ConfigScope::Platform,
        &definition,
        "8080",
        None,
        Revision::new(0),
    ));
    let tenant = ok_or_panic(ConfigValue::new(
        None,
        ConfigScope::Tenant(tenant_id),
        &definition,
        "3000",
        None,
        Revision::new(0),
    ));
    let module_scope = ConfigScope::Module {
        tenant_id,
        module: "api".to_string(),
    };
    let module_value = ok_or_panic(ConfigValue::new(
        None,
        module_scope.clone(),
        &definition,
        "4000",
        None,
        Revision::new(0),
    ));

    ok_or_panic(repo.save_value(&platform, &ctx).await);
    ok_or_panic(repo.save_value(&tenant, &ctx).await);
    ok_or_panic(repo.save_value(&module_value, &ctx).await);

    assert_eq!(
        ok_or_panic(
            repo.resolve("http.port", Some(tenant_id), Some("api"), &ctx)
                .await
        ),
        Some("4000".to_string())
    );
    assert_eq!(
        ok_or_panic(repo.resolve("http.port", Some(tenant_id), None, &ctx).await),
        Some("3000".to_string())
    );
    assert_eq!(
        ok_or_panic(repo.resolve("http.port", None, None, &any_ctx()).await),
        Some("8080".to_string())
    );

    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn sensitive_value_rejects_plain_value_and_returns_secret_ref(
    pool: sqlx::PgPool,
) -> sqlx::Result<()> {
    let repo = PostgresConfigurationRepository::new(pool.clone(), SystemClock, SystemRandom);
    let definition = secret_definition("db.password");
    ok_or_panic(repo.save_definition(&definition).await);

    let value = ok_or_panic(ConfigValue::new(
        None,
        ConfigScope::Platform,
        &definition,
        "${secret:db.password}",
        Some("${secret:db.password}".to_string()),
        Revision::new(0),
    ));
    let saved = ok_or_panic(repo.save_value(&value, &any_ctx()).await);
    assert_eq!(saved.effective_value(), "${secret:db.password}");

    let fetched = some_or_panic(ok_or_panic(
        repo.get_value("db.password", &ConfigScope::Platform, &any_ctx())
            .await,
    ));
    assert_eq!(fetched.effective_value(), "${secret:db.password}");

    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn invalid_value_does_not_replace_existing_snapshot(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let repo = PostgresConfigurationRepository::new(pool.clone(), SystemClock, SystemRandom);
    let definition = int_definition("http.port", "8080");
    ok_or_panic(repo.save_definition(&definition).await);

    let value = ok_or_panic(ConfigValue::new(
        None,
        ConfigScope::Platform,
        &definition,
        "3000",
        None,
        Revision::new(0),
    ));
    let saved = ok_or_panic(repo.save_value(&value, &any_ctx()).await);

    let bad_update = ConfigValue {
        id: saved.id,
        scope: ConfigScope::Platform,
        config_key: "http.port".to_string(),
        raw_value: "not-an-integer".to_string(),
        secret_ref: None,
        revision: saved.revision,
    };
    assert!(repo.save_value(&bad_update, &any_ctx()).await.is_err());

    let fetched = some_or_panic(ok_or_panic(
        repo.get_value("http.port", &ConfigScope::Platform, &any_ctx())
            .await,
    ));
    assert_eq!(fetched.raw_value, "3000");

    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn stale_revision_update_is_rejected(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let repo = PostgresConfigurationRepository::new(pool.clone(), SystemClock, SystemRandom);
    let definition = int_definition("http.port", "8080");
    ok_or_panic(repo.save_definition(&definition).await);

    let value = ok_or_panic(ConfigValue::new(
        None,
        ConfigScope::Platform,
        &definition,
        "3000",
        None,
        Revision::new(0),
    ));
    let saved = ok_or_panic(repo.save_value(&value, &any_ctx()).await);

    let valid_update = ConfigValue {
        id: saved.id,
        scope: ConfigScope::Platform,
        config_key: "http.port".to_string(),
        raw_value: "4000".to_string(),
        secret_ref: None,
        revision: saved.revision,
    };
    let _ = ok_or_panic(repo.save_value(&valid_update, &any_ctx()).await);

    let stale = ConfigValue {
        id: saved.id,
        scope: ConfigScope::Platform,
        config_key: "http.port".to_string(),
        raw_value: "5000".to_string(),
        secret_ref: None,
        revision: saved.revision,
    };
    assert!(repo.save_value(&stale, &any_ctx()).await.is_err());

    Ok(())
}
