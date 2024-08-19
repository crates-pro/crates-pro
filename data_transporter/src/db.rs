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
                    IF EXISTS (SELECT 1 FROM pg_tables WHERE tablename = 'program_versions') THEN
                        DROP TABLE program_versions CASCADE;
                    END IF;
                    
                    IF EXISTS (SELECT 1 FROM pg_tables WHERE tablename = 'programs') THEN
                        DROP TABLE programs CASCADE;
                    END IF;
                END $$;
                ",
            )
            .await
    }

    pub async fn create_tables(&self) -> Result<(), Error> {
        self.client
            .batch_execute(
                "
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
    
            CREATE TABLE IF NOT EXISTS program_versions (
                name_and_version TEXT PRIMARY KEY,
                id TEXT NOT NULL,
                name TEXT NOT NULL,
                version TEXT NOT NULL,
                documentation TEXT,
                version_type TEXT NOT NULL,
                created_at TIMESTAMPTZ DEFAULT NOW()
            );
            ",
            )
            .await
    }

    pub async fn insert_program_data(
        &self,
        program: Program,
        uprogram: UProgram,
        versions: Vec<UVersion>,
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

        // 插入 UVersion 数据
        for version in versions {
            match version {
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
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use model::tugraph_model::{ApplicationVersion, Library, LibraryVersion};
    use tokio_postgres::{Error, NoTls};
    use uuid::Uuid;

    #[tokio::test]
    async fn test_insert_program_data() -> Result<(), Error> {
        // 连接到测试数据库
        let db_handler = DBHandler::connect().await?;

        // 清除表数据
        db_handler
            .client
            .batch_execute("TRUNCATE TABLE program_versions, programs CASCADE")
            .await?;

        // 创建表
        db_handler.create_tables().await?;

        // 准备测试数据
        let program_id = Uuid::new_v4().to_string();
        let program = Program {
            id: program_id.clone(),
            name: "Test Program".to_string(),
            description: Some("A test program".to_string()),
            namespace: Some("test".to_string()),
            max_version: Some("1.0.0".to_string()),
            github_url: Some("http://github.com/test".to_string()),
            mega_url: Some("http://mega.nz/test".to_string()),
            doc_url: Some("http://docs.rs/test".to_string()),
        };

        let uprogram = UProgram::Library(Library {
            id: program_id.clone(),
            name: "Test Program".to_string(),
            downloads: 100,
            cratesio: Some("http://crates.io/test".to_string()),
        });

        let versions = vec![
            UVersion::LibraryVersion(LibraryVersion {
                id: Uuid::new_v4().to_string(),
                name_and_version: "Test Program v1.0.0".to_string(),
                name: "Test Program".to_string(),
                version: "1.0.0".to_string(),
                documentation: "http://docs.rs/test/1.0.0".to_string(),
            }),
            UVersion::ApplicationVersion(ApplicationVersion {
                id: Uuid::new_v4().to_string(),
                name_and_version: "Test Program v1.1.0".to_string(),
                name: "Test Program".to_string(),
                version: "1.1.0".to_string(),
            }),
        ];

        // 插入数据
        db_handler
            .insert_program_data(program, uprogram, versions.clone())
            .await?;

        // 验证插入
        let rows = db_handler
            .client
            .query(r#"SELECT id FROM programs WHERE id = \$1"#, &[&program_id])
            .await?;
        assert_eq!(rows.len(), 1, "程序未正确插入");

        let rows = db_handler
            .client
            .query(
                r#"SELECT id FROM program_versions WHERE program_id = \$1"#,
                &[&program_id],
            )
            .await?;
        assert_eq!(rows.len(), versions.len(), "版本未正确插入");

        Ok(())
    }
}
