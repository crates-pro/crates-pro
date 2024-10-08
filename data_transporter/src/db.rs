use model::tugraph_model::{Program, UProgram, UVersion};
use tokio_postgres::{Error, NoTls};

pub struct DBHandler {
    client: tokio_postgres::Client,
}

impl DBHandler {
    pub async fn connect() -> Result<Self, Error> {
        let (client, connection) = tokio_postgres::connect(
            "host=172.17.0.1 port=30432 user=mega password=mega dbname=postgres",
            NoTls,
        )
        .await?;

        // Spawn the connection on a separate task
        tokio::spawn(async move {
            if let Err(e) = connection.await {
                eprintln!("connection error: {}", e);
            }
        });

        // 创建 cratespro 数据库
        client
            .execute("CREATE DATABASE cratespro", &[])
            .await
            .or_else(|err| {
                if let Some(db_err) = err.as_db_error() {
                    if db_err.code() == &tokio_postgres::error::SqlState::DUPLICATE_DATABASE {
                        return Ok(0);
                    }
                }
                Err(err)
            })?;

        // 重新连接到 cratespro 数据库
        let (client, connection) = tokio_postgres::connect(
            "host=172.17.0.1 port=30432 user=mega password=mega dbname=cratespro",
            NoTls,
        )
        .await?;

        // Spawn the connection on a separate task
        tokio::spawn(async move {
            if let Err(e) = connection.await {
                eprintln!("connection error: {}", e);
            }
        });

        Ok(DBHandler { client })
    }

    pub async fn clear_database(&self) -> Result<(), Error> {
        self.client
            .batch_execute(
                "
                DO $$
                BEGIN
                    IF EXISTS (SELECT 1 FROM pg_tables WHERE tablename = 'programs') THEN
                        DROP TABLE programs CASCADE;
                    END IF;


                    IF EXISTS (SELECT 1 FROM pg_tables WHERE tablename = 'program_versions') THEN
                        DROP TABLE program_versions CASCADE;
                    END IF;

                    IF EXISTS (SELECT 1 FROM pg_tables WHERE tablename = 'program_dependencies') THEN
                        DROP TABLE program_dependencies CASCADE;
                    END IF;
                    

                END $$;
                ",
            )
            .await
    }

    pub async fn create_tables(&self) -> Result<(), Error> {
        let create_programs_table = "
            CREATE TABLE IF NOT EXISTS programs (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                description TEXT,
                namespace TEXT,
                max_version TEXT,
                github_url TEXT,
                mega_url TEXT,
                doc_url TEXT,
                program_type TEXT NOT NULL,
                downloads BIGINT,
                cratesio TEXT
            );
        ";

        let create_program_versions_table = "
            CREATE TABLE IF NOT EXISTS program_versions (
                name_and_version TEXT PRIMARY KEY,
                id TEXT NOT NULL,
                name TEXT NOT NULL,
                version TEXT NOT NULL,
                documentation TEXT,
                version_type TEXT NOT NULL,
                created_at TIMESTAMPTZ DEFAULT NOW()
            );
        ";

        let create_program_dependencies_table = "
            CREATE TABLE IF NOT EXISTS program_dependencies (
                name_and_version TEXT NOT NULL,
                dependency_name TEXT NOT NULL,
                dependency_version TEXT NOT NULL,
                PRIMARY KEY (name_and_version, dependency_name, dependency_version)
            );
        ";

        // 执行创建表的 SQL 语句
        let result = self
            .client
            .batch_execute(&format!(
                "{}{}{}",
                create_programs_table,
                create_program_versions_table,
                create_program_dependencies_table
            ))
            .await;

        match result {
            Ok(_) => {
                tracing::info!("Tables created successfully.");
                Ok(())
            }
            Err(e) => {
                tracing::error!("Error creating tables: {:?}", e);
                Err(e)
            }
        }
    }
    pub async fn insert_program_data(
        &self,
        program: Program,
        uprogram: UProgram,
        versions: Vec<crate::VersionInfo>,
    ) -> Result<(), Error> {
        let (program_type, downloads, cratesio) = match &uprogram {
            UProgram::Library(lib) => ("Library", Some(lib.downloads), lib.cratesio.clone()),
            UProgram::Application(_) => ("Application", None, None),
        };

        self.client
            .execute(
                "
            INSERT INTO programs (
                id, name, description, namespace, 
                max_version, github_url, mega_url, doc_url,
                program_type, downloads, cratesio
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            ",
                &[
                    &program.id,
                    &program.name,
                    &program.description.unwrap_or_default(),
                    &program.namespace.unwrap_or_default(),
                    &program.max_version.unwrap_or_default(),
                    &program.github_url.unwrap_or_default(),
                    &program.mega_url.unwrap_or_default(),
                    &program.doc_url.unwrap_or_default(),
                    &program_type,
                    &downloads.unwrap_or_default(),
                    &cratesio.unwrap_or_default(),
                ],
            )
            .await
            .map_err(|e| {
                eprintln!("Error inserting program: {:?}", e);
                e
            })
            .unwrap();

        tracing::info!("finish to insert program.");

        // 插入 UVersion 数据
        for version in versions {
            let name_and_version = version.version_base.get_name_and_version();

            match version.version_base {
                UVersion::LibraryVersion(lib_ver) => {
                    self.client
                        .execute(
                            "
                        INSERT INTO program_versions (
                            name_and_version, id, name, version, 
                            documentation, version_type, created_at
                        ) VALUES ($1, $2, $3, $4, $5, $6, NOW())
                        ",
                            &[
                                &lib_ver.name_and_version,
                                &lib_ver.id,
                                &lib_ver.name,
                                &lib_ver.version,
                                &Some(lib_ver.documentation),
                                &"LibraryVersion",
                            ],
                        )
                        .await
                        .unwrap();
                }
                UVersion::ApplicationVersion(app_ver) => {
                    self.client
                        .execute(
                            "
                        INSERT INTO program_versions (
                            name_and_version, id, name, version, 
                            documentation, version_type, created_at
                        ) VALUES ($1, $2, $3, $4, $5, $6, NOW())
                        ",
                            &[
                                &app_ver.name_and_version,
                                &app_ver.id,
                                &app_ver.name,
                                &app_ver.version,
                                &None::<String>, // ApplicationVersion 没有 documentation 字段
                                &"ApplicationVersion",
                            ],
                        )
                        .await
                        .unwrap();
                }
            }

            // 插入该版本的所有依赖项
            for dep in version.dependencies {
                self.client
                    .execute(
                        "
                        INSERT INTO program_dependencies (
                            name_and_version, dependency_name, dependency_version
                        ) VALUES ($1, $2, $3)
                        ",
                        &[&name_and_version, &dep.name, &dep.version],
                    )
                    .await?;
            }
        }
        tracing::info!("Finish to insert all versions.");

        Ok(())
    }
}
