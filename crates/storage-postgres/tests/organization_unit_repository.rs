use domain_organization::organization_unit::OrganizationUnit;
use domain_organization::tenant::Tenant;
use foundation::{
    Clock, OrganizationId, RequestContext, Revision, SystemClock, TenantId, uuid::Uuid,
};
use storage_api::{ListOptions, OrganizationUnitRepository, TenantRepository};
use storage_postgres::organization_unit_repository::PostgresOrganizationUnitRepository;
use storage_postgres::tenant_repository::PostgresTenantRepository;

fn parse_uuid(s: &str) -> Uuid {
    match Uuid::parse_str(s) {
        Ok(v) => v,
        Err(e) => panic!("{e}"),
    }
}

fn parse_tenant(s: &str) -> TenantId {
    match TenantId::parse_str(s) {
        Ok(v) => v,
        Err(e) => panic!("{e}"),
    }
}

fn parse_org(s: &str) -> OrganizationId {
    match OrganizationId::parse_str(s) {
        Ok(v) => v,
        Err(e) => panic!("{e}"),
    }
}

fn tenant_ctx(tenant: &str) -> RequestContext {
    RequestContext {
        tenant_id: Some(parse_tenant(tenant)),
        ..Default::default()
    }
}

fn ok_or_panic<T, E: std::fmt::Display>(result: Result<T, E>) -> T {
    match result {
        Ok(v) => v,
        Err(e) => panic!("{e}"),
    }
}

async fn create_tenant(
    repo: &PostgresTenantRepository,
    id: Uuid,
    code: &str,
    name: &str,
) -> Tenant {
    let tenant = ok_or_panic(Tenant::new(
        parse_tenant(&id.to_string()),
        code,
        name,
        Option::<&str>::None,
        Option::<&str>::None,
        &SystemClock,
        None,
    ));
    let ctx = tenant_ctx(&id.to_string());
    ok_or_panic(repo.create(&tenant, &ctx).await);
    tenant
}

fn new_unit(
    tenant: &Tenant,
    id: Uuid,
    parent: Option<OrganizationId>,
    code: &str,
    name: &str,
) -> OrganizationUnit {
    ok_or_panic(OrganizationUnit::new(
        parse_org(&id.to_string()),
        tenant.id,
        parent,
        code,
        name,
        &SystemClock,
        None,
    ))
}

#[sqlx::test(migrations = "../../migrations")]
async fn create_root_and_child_builds_closure(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let tenant_repo = PostgresTenantRepository::new(pool.clone());
    let unit_repo = PostgresOrganizationUnitRepository::new(pool.clone());
    let tenant = create_tenant(
        &tenant_repo,
        parse_uuid("018e1234-5678-7abc-8def-0123456789ab"),
        "acme",
        "Acme",
    )
    .await;
    let ctx = tenant_ctx("018e1234-5678-7abc-8def-0123456789ab");

    let root_id = parse_uuid("018e0000-0000-0000-0000-000000000001");
    let child_id = parse_uuid("018e0000-0000-0000-0000-000000000002");
    let root = new_unit(&tenant, root_id, None, "root", "Root");
    let child = new_unit(
        &tenant,
        child_id,
        Some(parse_org(&root_id.to_string())),
        "child",
        "Child",
    );

    ok_or_panic(unit_repo.create(&root, &ctx).await);
    ok_or_panic(unit_repo.create(&child, &ctx).await);

    let closure_count: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM org.organization_unit_closure WHERE tenant_id = $1")
            .bind(tenant.id.as_uuid())
            .fetch_one(&pool)
            .await?;
    assert_eq!(closure_count.0, 3); // root->root, child->child, root->child

    let root_to_child: (i32,) = sqlx::query_as(
        "SELECT depth FROM org.organization_unit_closure
         WHERE tenant_id = $1 AND ancestor_id = $2 AND descendant_id = $3",
    )
    .bind(tenant.id.as_uuid())
    .bind(root_id)
    .bind(child_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(root_to_child.0, 1);

    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn move_subtree_to_new_parent_updates_closure(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let tenant_repo = PostgresTenantRepository::new(pool.clone());
    let unit_repo = PostgresOrganizationUnitRepository::new(pool.clone());
    let tenant = create_tenant(
        &tenant_repo,
        parse_uuid("018e1234-5678-7abc-8def-0123456789ab"),
        "acme",
        "Acme",
    )
    .await;
    let ctx = tenant_ctx("018e1234-5678-7abc-8def-0123456789ab");

    let root_a = parse_uuid("018e0000-0000-0000-0000-000000000001");
    let root_b = parse_uuid("018e0000-0000-0000-0000-000000000002");
    let child = parse_uuid("018e0000-0000-0000-0000-000000000003");
    let grandchild = parse_uuid("018e0000-0000-0000-0000-000000000004");

    ok_or_panic(
        unit_repo
            .create(&new_unit(&tenant, root_a, None, "a", "A"), &ctx)
            .await,
    );
    ok_or_panic(
        unit_repo
            .create(&new_unit(&tenant, root_b, None, "b", "B"), &ctx)
            .await,
    );
    ok_or_panic(
        unit_repo
            .create(
                &new_unit(
                    &tenant,
                    child,
                    Some(parse_org(&root_a.to_string())),
                    "child",
                    "Child",
                ),
                &ctx,
            )
            .await,
    );
    ok_or_panic(
        unit_repo
            .create(
                &new_unit(
                    &tenant,
                    grandchild,
                    Some(parse_org(&child.to_string())),
                    "grandchild",
                    "Grandchild",
                ),
                &ctx,
            )
            .await,
    );

    let mut to_move = ok_or_panic(unit_repo.by_id(parse_org(&child.to_string()), &ctx).await);
    let descendants = vec![
        parse_org(&child.to_string()),
        parse_org(&grandchild.to_string()),
    ];
    ok_or_panic(to_move.set_parent(
        Some(parse_org(&root_b.to_string())),
        &descendants,
        &SystemClock,
        None,
    ));
    ok_or_panic(
        unit_repo
            .update(&to_move, to_move.revision.prev(), &ctx)
            .await,
    );

    let b_to_grandchild: (i32,) = sqlx::query_as(
        "SELECT depth FROM org.organization_unit_closure
         WHERE tenant_id = $1 AND ancestor_id = $2 AND descendant_id = $3",
    )
    .bind(tenant.id.as_uuid())
    .bind(root_b)
    .bind(grandchild)
    .fetch_one(&pool)
    .await?;
    assert_eq!(b_to_grandchild.0, 2);

    let a_to_grandchild: Option<(i32,)> = sqlx::query_as(
        "SELECT depth FROM org.organization_unit_closure
         WHERE tenant_id = $1 AND ancestor_id = $2 AND descendant_id = $3",
    )
    .bind(tenant.id.as_uuid())
    .bind(root_a)
    .bind(grandchild)
    .fetch_optional(&pool)
    .await?;
    assert!(a_to_grandchild.is_none());

    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn move_under_self_or_descendant_fails(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let tenant_repo = PostgresTenantRepository::new(pool.clone());
    let unit_repo = PostgresOrganizationUnitRepository::new(pool.clone());
    let tenant = create_tenant(
        &tenant_repo,
        parse_uuid("018e1234-5678-7abc-8def-0123456789ab"),
        "acme",
        "Acme",
    )
    .await;
    let ctx = tenant_ctx("018e1234-5678-7abc-8def-0123456789ab");

    let root = parse_uuid("018e0000-0000-0000-0000-000000000001");
    let parent = parse_uuid("018e0000-0000-0000-0000-000000000002");
    let child = parse_uuid("018e0000-0000-0000-0000-000000000003");

    ok_or_panic(
        unit_repo
            .create(&new_unit(&tenant, root, None, "root", "Root"), &ctx)
            .await,
    );
    ok_or_panic(
        unit_repo
            .create(
                &new_unit(
                    &tenant,
                    parent,
                    Some(parse_org(&root.to_string())),
                    "parent",
                    "Parent",
                ),
                &ctx,
            )
            .await,
    );
    ok_or_panic(
        unit_repo
            .create(
                &new_unit(
                    &tenant,
                    child,
                    Some(parse_org(&parent.to_string())),
                    "child",
                    "Child",
                ),
                &ctx,
            )
            .await,
    );

    let mut parent_unit = ok_or_panic(unit_repo.by_id(parse_org(&parent.to_string()), &ctx).await);
    let result = parent_unit.set_parent(
        Some(parse_org(&child.to_string())),
        &[
            parse_org(&parent.to_string()),
            parse_org(&child.to_string()),
        ],
        &SystemClock,
        None,
    );
    assert!(result.is_err());

    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn delete_with_children_fails(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let tenant_repo = PostgresTenantRepository::new(pool.clone());
    let unit_repo = PostgresOrganizationUnitRepository::new(pool.clone());
    let tenant = create_tenant(
        &tenant_repo,
        parse_uuid("018e1234-5678-7abc-8def-0123456789ab"),
        "acme",
        "Acme",
    )
    .await;
    let ctx = tenant_ctx("018e1234-5678-7abc-8def-0123456789ab");

    let parent = parse_uuid("018e0000-0000-0000-0000-000000000001");
    let child = parse_uuid("018e0000-0000-0000-0000-000000000002");

    ok_or_panic(
        unit_repo
            .create(&new_unit(&tenant, parent, None, "parent", "Parent"), &ctx)
            .await,
    );
    ok_or_panic(
        unit_repo
            .create(
                &new_unit(
                    &tenant,
                    child,
                    Some(parse_org(&parent.to_string())),
                    "child",
                    "Child",
                ),
                &ctx,
            )
            .await,
    );

    let result = unit_repo
        .delete(
            parse_org(&parent.to_string()),
            Revision::initial(),
            SystemClock.now(),
            &ctx,
        )
        .await;
    assert!(result.is_err());

    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn delete_leaf_removes_closure(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let tenant_repo = PostgresTenantRepository::new(pool.clone());
    let unit_repo = PostgresOrganizationUnitRepository::new(pool.clone());
    let tenant = create_tenant(
        &tenant_repo,
        parse_uuid("018e1234-5678-7abc-8def-0123456789ab"),
        "acme",
        "Acme",
    )
    .await;
    let ctx = tenant_ctx("018e1234-5678-7abc-8def-0123456789ab");

    let root = parse_uuid("018e0000-0000-0000-0000-000000000001");
    let leaf = parse_uuid("018e0000-0000-0000-0000-000000000002");

    ok_or_panic(
        unit_repo
            .create(&new_unit(&tenant, root, None, "root", "Root"), &ctx)
            .await,
    );
    ok_or_panic(
        unit_repo
            .create(
                &new_unit(
                    &tenant,
                    leaf,
                    Some(parse_org(&root.to_string())),
                    "leaf",
                    "Leaf",
                ),
                &ctx,
            )
            .await,
    );

    ok_or_panic(
        unit_repo
            .delete(
                parse_org(&leaf.to_string()),
                Revision::initial(),
                SystemClock.now(),
                &ctx,
            )
            .await,
    );

    let root_to_leaf: Option<(i32,)> = sqlx::query_as(
        "SELECT 1 FROM org.organization_unit_closure
         WHERE tenant_id = $1 AND ancestor_id = $2 AND descendant_id = $3",
    )
    .bind(tenant.id.as_uuid())
    .bind(root)
    .bind(leaf)
    .fetch_optional(&pool)
    .await?;
    assert!(root_to_leaf.is_none());

    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn cross_tenant_parent_is_rejected(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let tenant_repo = PostgresTenantRepository::new(pool.clone());
    let unit_repo = PostgresOrganizationUnitRepository::new(pool.clone());

    let tenant_a = create_tenant(
        &tenant_repo,
        parse_uuid("018e1234-5678-7abc-8def-0123456789ab"),
        "a",
        "A",
    )
    .await;
    let tenant_b = create_tenant(
        &tenant_repo,
        parse_uuid("018f1234-5678-7abc-8def-0123456789ab"),
        "b",
        "B",
    )
    .await;

    let ctx_a = tenant_ctx("018e1234-5678-7abc-8def-0123456789ab");
    let ctx_b = tenant_ctx("018f1234-5678-7abc-8def-0123456789ab");

    let root_a = parse_uuid("018e0000-0000-0000-0000-000000000001");
    let child_b = parse_uuid("018e0000-0000-0000-0000-000000000002");

    ok_or_panic(
        unit_repo
            .create(
                &new_unit(&tenant_a, root_a, None, "root-a", "Root A"),
                &ctx_a,
            )
            .await,
    );

    let child = new_unit(
        &tenant_b,
        child_b,
        Some(parse_org(&root_a.to_string())),
        "child-b",
        "Child B",
    );
    let result = unit_repo.create(&child, &ctx_b).await;
    assert!(result.is_err());

    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn list_orders_by_code_with_bound(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let tenant_repo = PostgresTenantRepository::new(pool.clone());
    let unit_repo = PostgresOrganizationUnitRepository::new(pool.clone());
    let tenant = create_tenant(
        &tenant_repo,
        parse_uuid("018e1234-5678-7abc-8def-0123456789ab"),
        "acme",
        "Acme",
    )
    .await;
    let ctx = tenant_ctx("018e1234-5678-7abc-8def-0123456789ab");

    let first = parse_uuid("018e0000-0000-0000-0000-000000000001");
    let second = parse_uuid("018e0000-0000-0000-0000-000000000002");

    ok_or_panic(
        unit_repo
            .create(&new_unit(&tenant, second, None, "zulu", "Zulu"), &ctx)
            .await,
    );
    ok_or_panic(
        unit_repo
            .create(&new_unit(&tenant, first, None, "alpha", "Alpha"), &ctx)
            .await,
    );

    let page = ok_or_panic(unit_repo.list(&ctx, ListOptions::default()).await);
    let codes: Vec<&str> = page.items.iter().map(|u| u.code.as_str()).collect();
    assert_eq!(codes, vec!["alpha", "zulu"]);

    Ok(())
}
