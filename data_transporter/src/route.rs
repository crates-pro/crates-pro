use crate::data_reader::DataReader;
use actix_web::{web, HttpResponse, Responder};

#[derive(Clone)]
pub struct ApiHandler {
    reader: DataReader,
}

impl ApiHandler {
    pub async fn new(reader: DataReader) -> Self {
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
}
