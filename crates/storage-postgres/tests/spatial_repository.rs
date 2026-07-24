use domain_organization::spatial::{Area, Building, Floor, Site};
use domain_organization::tenant::Tenant;
use foundation::{
    AreaId, BuildingId, Clock, FloorId, RequestContext, Revision, SiteId, SystemClock, TenantId,
    uuid::Uuid,
};
use storage_api::{ListOptions, SpatialRepository, TenantRepository};
use storage_postgres::spatial_repository::PostgresSpatialRepository;
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

fn parse_site(s: &str) -> SiteId {
    match SiteId::parse_str(s) {
        Ok(v) => v,
        Err(e) => panic!("{e}"),
    }
}

fn parse_building(s: &str) -> BuildingId {
    match BuildingId::parse_str(s) {
        Ok(v) => v,
        Err(e) => panic!("{e}"),
    }
}

fn parse_floor(s: &str) -> FloorId {
    match FloorId::parse_str(s) {
        Ok(v) => v,
        Err(e) => panic!("{e}"),
    }
}

fn parse_area(s: &str) -> AreaId {
    match AreaId::parse_str(s) {
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

fn new_site(tenant: &Tenant, id: Uuid, code: &str, name: &str) -> Site {
    ok_or_panic(Site::new(
        parse_site(&id.to_string()),
        tenant.id,
        None,
        code,
        name,
        "1 Main St",
        &SystemClock,
        None,
    ))
}

fn new_building(tenant: &Tenant, id: Uuid, site_id: SiteId, code: &str, name: &str) -> Building {
    ok_or_panic(Building::new(
        parse_building(&id.to_string()),
        tenant.id,
        site_id,
        code,
        name,
        &SystemClock,
        None,
    ))
}

fn new_floor(
    tenant: &Tenant,
    id: Uuid,
    building_id: BuildingId,
    code: &str,
    name: &str,
    level: i32,
) -> Floor {
    ok_or_panic(Floor::new(
        parse_floor(&id.to_string()),
        tenant.id,
        building_id,
        code,
        name,
        level,
        &SystemClock,
        None,
    ))
}

fn new_area(
    tenant: &Tenant,
    id: Uuid,
    floor_id: Option<FloorId>,
    parent_id: Option<AreaId>,
    code: &str,
    name: &str,
) -> Area {
    ok_or_panic(Area::new(
        parse_area(&id.to_string()),
        tenant.id,
        floor_id,
        parent_id,
        code,
        name,
        &SystemClock,
        None,
    ))
}

#[sqlx::test(migrations = "../../migrations")]
async fn create_site_building_floor_area_round_trip(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let tenant_repo = PostgresTenantRepository::new(pool.clone());
    let repo = PostgresSpatialRepository::new(pool.clone());
    let tenant = create_tenant(
        &tenant_repo,
        parse_uuid("018e1234-5678-7abc-8def-0123456789ab"),
        "acme",
        "Acme",
    )
    .await;
    let ctx = tenant_ctx("018e1234-5678-7abc-8def-0123456789ab");

    let site_id = parse_uuid("018e0000-0000-0000-0000-000000000001");
    let building_id = parse_uuid("018e0000-0000-0000-0000-000000000002");
    let floor_id = parse_uuid("018e0000-0000-0000-0000-000000000003");
    let area_id = parse_uuid("018e0000-0000-0000-0000-000000000004");

    let site = new_site(&tenant, site_id, "hq", "HQ");
    ok_or_panic(repo.create_site(&site, &ctx).await);

    let building = new_building(
        &tenant,
        building_id,
        parse_site(&site_id.to_string()),
        "b1",
        "Building 1",
    );
    ok_or_panic(repo.create_building(&building, &ctx).await);

    let floor = new_floor(
        &tenant,
        floor_id,
        parse_building(&building_id.to_string()),
        "f1",
        "Floor 1",
        1,
    );
    ok_or_panic(repo.create_floor(&floor, &ctx).await);

    let mut area = new_area(
        &tenant,
        area_id,
        Some(parse_floor(&floor_id.to_string())),
        None,
        "lobby",
        "Lobby",
    );
    ok_or_panic(area.set_coordinates("WGS84", Some(0.0), Some(0.0), None, &SystemClock, None));
    ok_or_panic(repo.create_area(&area, &ctx).await);

    let read = ok_or_panic(
        repo.area_by_id(parse_area(&area_id.to_string()), &ctx)
            .await,
    );
    assert_eq!(read.code, "lobby");
    assert_eq!(read.latitude, Some(0.0));

    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn duplicate_site_code_fails(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let tenant_repo = PostgresTenantRepository::new(pool.clone());
    let repo = PostgresSpatialRepository::new(pool.clone());
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
        repo.create_site(&new_site(&tenant, first, "hq", "HQ"), &ctx)
            .await,
    );
    let result = repo
        .create_site(&new_site(&tenant, second, "hq", "HQ2"), &ctx)
        .await;
    assert!(result.is_err());

    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn building_requires_existing_site(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let tenant_repo = PostgresTenantRepository::new(pool.clone());
    let repo = PostgresSpatialRepository::new(pool.clone());
    let tenant = create_tenant(
        &tenant_repo,
        parse_uuid("018e1234-5678-7abc-8def-0123456789ab"),
        "acme",
        "Acme",
    )
    .await;
    let ctx = tenant_ctx("018e1234-5678-7abc-8def-0123456789ab");

    let building = new_building(
        &tenant,
        parse_uuid("018e0000-0000-0000-0000-000000000002"),
        parse_site("018e0000-0000-0000-0000-000000000001"),
        "b1",
        "Building 1",
    );
    let result = repo.create_building(&building, &ctx).await;
    assert!(result.is_err());

    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn floor_requires_existing_building(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let tenant_repo = PostgresTenantRepository::new(pool.clone());
    let repo = PostgresSpatialRepository::new(pool.clone());
    let tenant = create_tenant(
        &tenant_repo,
        parse_uuid("018e1234-5678-7abc-8def-0123456789ab"),
        "acme",
        "Acme",
    )
    .await;
    let ctx = tenant_ctx("018e1234-5678-7abc-8def-0123456789ab");

    let floor = new_floor(
        &tenant,
        parse_uuid("018e0000-0000-0000-0000-000000000003"),
        parse_building("018e0000-0000-0000-0000-000000000002"),
        "f1",
        "Floor 1",
        1,
    );
    let result = repo.create_floor(&floor, &ctx).await;
    assert!(result.is_err());

    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn delete_floor_with_area_fails(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let tenant_repo = PostgresTenantRepository::new(pool.clone());
    let repo = PostgresSpatialRepository::new(pool.clone());
    let tenant = create_tenant(
        &tenant_repo,
        parse_uuid("018e1234-5678-7abc-8def-0123456789ab"),
        "acme",
        "Acme",
    )
    .await;
    let ctx = tenant_ctx("018e1234-5678-7abc-8def-0123456789ab");

    let site_id = parse_uuid("018e0000-0000-0000-0000-000000000001");
    let building_id = parse_uuid("018e0000-0000-0000-0000-000000000002");
    let floor_id = parse_uuid("018e0000-0000-0000-0000-000000000003");
    let area_id = parse_uuid("018e0000-0000-0000-0000-000000000004");

    ok_or_panic(
        repo.create_site(&new_site(&tenant, site_id, "hq", "HQ"), &ctx)
            .await,
    );
    ok_or_panic(
        repo.create_building(
            &new_building(
                &tenant,
                building_id,
                parse_site(&site_id.to_string()),
                "b1",
                "B1",
            ),
            &ctx,
        )
        .await,
    );
    ok_or_panic(
        repo.create_floor(
            &new_floor(
                &tenant,
                floor_id,
                parse_building(&building_id.to_string()),
                "f1",
                "F1",
                1,
            ),
            &ctx,
        )
        .await,
    );
    let mut area = new_area(
        &tenant,
        area_id,
        Some(parse_floor(&floor_id.to_string())),
        None,
        "lobby",
        "Lobby",
    );
    ok_or_panic(area.set_coordinates("WGS84", Some(0.0), Some(0.0), None, &SystemClock, None));
    ok_or_panic(repo.create_area(&area, &ctx).await);

    let result = repo
        .delete_floor(
            parse_floor(&floor_id.to_string()),
            Revision::initial(),
            SystemClock.now(),
            &ctx,
        )
        .await;
    assert!(result.is_err());

    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn area_move_updates_closure(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let tenant_repo = PostgresTenantRepository::new(pool.clone());
    let repo = PostgresSpatialRepository::new(pool.clone());
    let tenant = create_tenant(
        &tenant_repo,
        parse_uuid("018e1234-5678-7abc-8def-0123456789ab"),
        "acme",
        "Acme",
    )
    .await;
    let ctx = tenant_ctx("018e1234-5678-7abc-8def-0123456789ab");

    let floor_id = parse_uuid("018e0000-0000-0000-0000-000000000001");
    let root_a = parse_uuid("018e0000-0000-0000-0000-000000000002");
    let root_b = parse_uuid("018e0000-0000-0000-0000-000000000003");
    let child = parse_uuid("018e0000-0000-0000-0000-000000000004");

    ok_or_panic(
        repo.create_site(&new_site(&tenant, floor_id, "site", "Site"), &ctx)
            .await,
    );
    ok_or_panic(
        repo.create_building(
            &new_building(
                &tenant,
                parse_uuid("018e0000-0000-0000-0000-000000000005"),
                parse_site(&floor_id.to_string()),
                "b",
                "B",
            ),
            &ctx,
        )
        .await,
    );
    ok_or_panic(
        repo.create_floor(
            &new_floor(
                &tenant,
                parse_uuid("018e0000-0000-0000-0000-000000000006"),
                parse_building("018e0000-0000-0000-0000-000000000005"),
                "f",
                "F",
                1,
            ),
            &ctx,
        )
        .await,
    );
    let floor = parse_floor("018e0000-0000-0000-0000-000000000006");

    let root_a_area = new_area(&tenant, root_a, Some(floor), None, "a", "A");
    ok_or_panic(repo.create_area(&root_a_area, &ctx).await);

    let root_b_area = new_area(&tenant, root_b, Some(floor), None, "b", "B");
    ok_or_panic(repo.create_area(&root_b_area, &ctx).await);

    let mut child_area = new_area(
        &tenant,
        child,
        Some(floor),
        Some(parse_area(&root_a.to_string())),
        "c",
        "C",
    );
    ok_or_panic(repo.create_area(&child_area, &ctx).await);

    let descendants = vec![parse_area(&child.to_string())];
    ok_or_panic(child_area.set_parent(
        Some(parse_area(&root_b.to_string())),
        &descendants,
        &SystemClock,
        None,
    ));
    ok_or_panic(
        repo.update_area(&child_area, child_area.revision.prev(), &ctx)
            .await,
    );

    let b_to_child: (i32,) = sqlx::query_as(
        "SELECT depth FROM org.area_closure
         WHERE tenant_id = $1 AND ancestor_id = $2 AND descendant_id = $3",
    )
    .bind(tenant.id.as_uuid())
    .bind(root_b)
    .bind(child)
    .fetch_one(&pool)
    .await?;
    assert_eq!(b_to_child.0, 1);

    let a_to_child: Option<(i32,)> = sqlx::query_as(
        "SELECT depth FROM org.area_closure
         WHERE tenant_id = $1 AND ancestor_id = $2 AND descendant_id = $3",
    )
    .bind(tenant.id.as_uuid())
    .bind(root_a)
    .bind(child)
    .fetch_optional(&pool)
    .await?;
    assert!(a_to_child.is_none());

    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn area_move_under_descendant_fails(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let tenant_repo = PostgresTenantRepository::new(pool.clone());
    let repo = PostgresSpatialRepository::new(pool.clone());
    let tenant = create_tenant(
        &tenant_repo,
        parse_uuid("018e1234-5678-7abc-8def-0123456789ab"),
        "acme",
        "Acme",
    )
    .await;
    let ctx = tenant_ctx("018e1234-5678-7abc-8def-0123456789ab");

    let floor_id = parse_uuid("018e0000-0000-0000-0000-000000000001");
    let parent = parse_uuid("018e0000-0000-0000-0000-000000000002");
    let child = parse_uuid("018e0000-0000-0000-0000-000000000003");

    ok_or_panic(
        repo.create_site(&new_site(&tenant, floor_id, "site", "Site"), &ctx)
            .await,
    );
    ok_or_panic(
        repo.create_building(
            &new_building(
                &tenant,
                parse_uuid("018e0000-0000-0000-0000-000000000004"),
                parse_site(&floor_id.to_string()),
                "b",
                "B",
            ),
            &ctx,
        )
        .await,
    );
    ok_or_panic(
        repo.create_floor(
            &new_floor(
                &tenant,
                parse_uuid("018e0000-0000-0000-0000-000000000005"),
                parse_building("018e0000-0000-0000-0000-000000000004"),
                "f",
                "F",
                1,
            ),
            &ctx,
        )
        .await,
    );
    let floor = parse_floor("018e0000-0000-0000-0000-000000000005");

    let mut parent_area = new_area(&tenant, parent, Some(floor), None, "p", "P");
    ok_or_panic(repo.create_area(&parent_area, &ctx).await);

    let child_area = new_area(
        &tenant,
        child,
        Some(floor),
        Some(parse_area(&parent.to_string())),
        "c",
        "C",
    );
    ok_or_panic(repo.create_area(&child_area, &ctx).await);

    let result = parent_area.set_parent(
        Some(parse_area(&child.to_string())),
        &[
            parse_area(&parent.to_string()),
            parse_area(&child.to_string()),
        ],
        &SystemClock,
        None,
    );
    assert!(result.is_err());

    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn delete_area_with_children_fails(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let tenant_repo = PostgresTenantRepository::new(pool.clone());
    let repo = PostgresSpatialRepository::new(pool.clone());
    let tenant = create_tenant(
        &tenant_repo,
        parse_uuid("018e1234-5678-7abc-8def-0123456789ab"),
        "acme",
        "Acme",
    )
    .await;
    let ctx = tenant_ctx("018e1234-5678-7abc-8def-0123456789ab");

    let floor_id = parse_uuid("018e0000-0000-0000-0000-000000000001");
    let parent = parse_uuid("018e0000-0000-0000-0000-000000000002");
    let child = parse_uuid("018e0000-0000-0000-0000-000000000003");

    ok_or_panic(
        repo.create_site(&new_site(&tenant, floor_id, "site", "Site"), &ctx)
            .await,
    );
    ok_or_panic(
        repo.create_building(
            &new_building(
                &tenant,
                parse_uuid("018e0000-0000-0000-0000-000000000004"),
                parse_site(&floor_id.to_string()),
                "b",
                "B",
            ),
            &ctx,
        )
        .await,
    );
    ok_or_panic(
        repo.create_floor(
            &new_floor(
                &tenant,
                parse_uuid("018e0000-0000-0000-0000-000000000005"),
                parse_building("018e0000-0000-0000-0000-000000000004"),
                "f",
                "F",
                1,
            ),
            &ctx,
        )
        .await,
    );
    let floor = parse_floor("018e0000-0000-0000-0000-000000000005");

    let parent_area = new_area(&tenant, parent, Some(floor), None, "p", "P");
    ok_or_panic(repo.create_area(&parent_area, &ctx).await);

    let child_area = new_area(
        &tenant,
        child,
        Some(floor),
        Some(parse_area(&parent.to_string())),
        "c",
        "C",
    );
    ok_or_panic(repo.create_area(&child_area, &ctx).await);

    let result = repo
        .delete_area(
            parse_area(&parent.to_string()),
            Revision::initial(),
            SystemClock.now(),
            &ctx,
        )
        .await;
    assert!(result.is_err());

    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn area_range_query_returns_only_nearby(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let tenant_repo = PostgresTenantRepository::new(pool.clone());
    let repo = PostgresSpatialRepository::new(pool.clone());
    let tenant = create_tenant(
        &tenant_repo,
        parse_uuid("018e1234-5678-7abc-8def-0123456789ab"),
        "acme",
        "Acme",
    )
    .await;
    let ctx = tenant_ctx("018e1234-5678-7abc-8def-0123456789ab");

    let floor_id = parse_uuid("018e0000-0000-0000-0000-000000000001");
    let near_id = parse_uuid("018e0000-0000-0000-0000-000000000002");
    let far_id = parse_uuid("018e0000-0000-0000-0000-000000000003");

    ok_or_panic(
        repo.create_site(&new_site(&tenant, floor_id, "site", "Site"), &ctx)
            .await,
    );
    ok_or_panic(
        repo.create_building(
            &new_building(
                &tenant,
                parse_uuid("018e0000-0000-0000-0000-000000000004"),
                parse_site(&floor_id.to_string()),
                "b",
                "B",
            ),
            &ctx,
        )
        .await,
    );
    ok_or_panic(
        repo.create_floor(
            &new_floor(
                &tenant,
                parse_uuid("018e0000-0000-0000-0000-000000000005"),
                parse_building("018e0000-0000-0000-0000-000000000004"),
                "f",
                "F",
                1,
            ),
            &ctx,
        )
        .await,
    );
    let floor = parse_floor("018e0000-0000-0000-0000-000000000005");

    let mut near = new_area(&tenant, near_id, Some(floor), None, "near", "Near");
    ok_or_panic(near.set_coordinates("WGS84", Some(0.0), Some(0.0), None, &SystemClock, None));
    ok_or_panic(repo.create_area(&near, &ctx).await);

    let mut far = new_area(&tenant, far_id, Some(floor), None, "far", "Far");
    ok_or_panic(far.set_coordinates("WGS84", Some(10.0), Some(0.0), None, &SystemClock, None));
    ok_or_panic(repo.create_area(&far, &ctx).await);

    let page = ok_or_panic(
        repo.areas_within_radius(0.0, 0.0, 1000.0, &ctx, ListOptions::default())
            .await,
    );
    let codes: Vec<&str> = page.items.iter().map(|a| a.code.as_str()).collect();
    assert_eq!(codes, vec!["near"]);

    Ok(())
}
