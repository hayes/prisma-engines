mod gin;

use datamodel::parse_configuration;
use indoc::indoc;
use introspection_connector::{CompositeTypeDepth, IntrospectionConnector, IntrospectionContext};
use introspection_engine_tests::test_api::*;

#[test_connector(tags(CockroachDb))]
async fn introspecting_cockroach_db_with_postgres_provider(api: TestApi) {
    let setup = r#"
        CREATE TABLE "myTable" (
            id   INTEGER PRIMARY KEY,
            name STRING
       );
    "#;

    let schema = format!(
        r#"
        datasource mypg {{
            provider = "postgresql"
            url = "{}"
        }}

    "#,
        api.connection_string()
    );

    api.raw_cmd(setup).await;

    let ctx = IntrospectionContext {
        preview_features: Default::default(),
        source: parse_configuration(&schema)
            .unwrap()
            .subject
            .datasources
            .into_iter()
            .next()
            .unwrap(),
        composite_type_depth: CompositeTypeDepth::Infinite,
    };

    api.api
        .introspect(&datamodel::parse_datamodel(&schema).unwrap().subject, ctx)
        .await
        .unwrap();
}

#[test_connector(tags(CockroachDb))]
async fn rowid_introspects_to_autoincrement(api: TestApi) {
    let sql = r#"
    CREATE TABLE "myTable"(
        id   INT4 PRIMARY KEY DEFAULT unique_rowid(),
        name STRING NOT NULL
    );
    "#;

    api.raw_cmd(sql).await;

    let result = api.introspect_dml().await.unwrap();

    let expected = expect![[r#"
        model myTable {
          id   Int    @id @default(autoincrement())
          name String
        }
    "#]];

    expected.assert_eq(&result);
}

#[test_connector(tags(CockroachDb))]
async fn identity_introspects_to_sequence_with_default_settings(api: TestApi) {
    let sql = r#"
    CREATE TABLE "myTable" (
        id   INT4 GENERATED BY DEFAULT AS IDENTITY,
        name STRING NOT NULL,

        PRIMARY KEY (id)
    );
    "#;

    api.raw_cmd(sql).await;

    let result = api.introspect_dml().await.unwrap();

    let expected = expect![[r#"
        model myTable {
          id   Int    @id @default(sequence())
          name String
        }
    "#]];

    expected.assert_eq(&result);
}

#[test_connector(tags(CockroachDb))]
async fn identity_with_options_introspects_to_sequence_with_options(api: TestApi) {
    let sql = r#"
    CREATE TABLE "myTable" (
        id   INT4 GENERATED BY DEFAULT AS IDENTITY (MINVALUE 10 START 12 MAXVALUE 39 INCREMENT 3 CACHE 4),
        name STRING NOT NULL,

        PRIMARY KEY (id)
    );
    "#;

    api.raw_cmd(sql).await;

    let result = api.introspect_dml().await.unwrap();

    let expected = expect![[r#"
        model myTable {
          id   Int    @id @default(sequence(minValue: 10, maxValue: 39, cache: 4, increment: 3, start: 12))
          name String
        }
    "#]];

    expected.assert_eq(&result);
}

#[test_connector(tags(CockroachDb))]
async fn dbgenerated_type_casts_should_work(api: &TestApi) -> TestResult {
    api.barrel()
        .execute(move |migration| {
            migration.create_table("A", move |t| {
                t.inject_custom("id VARCHAR(30) PRIMARY KEY DEFAULT (now())::text");
            });
        })
        .await?;

    let dm = indoc! {r#"
        model A {
          id String @id @default(dbgenerated("now():::TIMESTAMPTZ::STRING")) @db.String(30)
        }
    "#};

    let result = api.introspect().await?;
    api.assert_eq_datamodels(dm, &result);

    Ok(())
}