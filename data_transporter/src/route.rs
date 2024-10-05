use crate::data_reader::DataReaderTrait;
use actix_multipart::Multipart;
use actix_web::{web, HttpResponse, Responder};
use sanitize_filename::sanitize;
use tokio::io::AsyncWriteExt;

pub struct ApiHandler {
    reader: Box<dyn DataReaderTrait>,
}

impl ApiHandler {
    pub async fn new(reader: Box<dyn DataReaderTrait>) -> Self {
        Self { reader }
    }

    pub async fn get_all_crates(&self) -> impl Responder {
        let program_ids = { self.reader.get_all_programs_id() }.await;
        HttpResponse::Ok().json(program_ids) // 返回 JSON 格式
    }

    pub async fn get_crate_details(&self, crate_name: web::Path<String>) -> impl Responder {
        match self.reader.get_program(&crate_name).await {
            Ok(program) => {
                match self.reader.get_type(&crate_name).await {
                    Ok((uprogram, islib)) => {
                        match self.reader.get_versions(&crate_name, islib).await {
                            Ok(versions) => {
                                HttpResponse::Ok().json((program, uprogram, versions))
                                // 返回 JSON 格式
                            }
                            Err(_) => {
                                HttpResponse::InternalServerError().body("Failed to get versions.")
                            }
                        }
                    }
                    Err(_) => HttpResponse::InternalServerError().body("Failed to get type."),
                }
            }
            Err(_) => HttpResponse::NotFound().body("Crate not found."),
        }
    }

    pub async fn upload_crate(mut payload: Multipart) -> impl Responder {
        use futures_util::StreamExt as _;
        let analysis_result = String::new();

        while let Some(Ok(mut field)) = payload.next().await {
            if let Some(content_disposition) = field.content_disposition() {
                if let Some(filename) = content_disposition.get_filename() {
                    let sanitized_filename = sanitize(filename);
                    let filepath = format!("/var/www/uploads/{}", sanitized_filename);
                    let mut f = tokio::fs::File::create(&filepath).await.unwrap();

                    while let Some(chunk) = field.next().await {
                        let data = chunk.unwrap();
                        f.write_all(&data).await.unwrap();
                    }

                    // analyze
                }
            }
        }

        HttpResponse::Ok().json(analysis_result)
    }
}
