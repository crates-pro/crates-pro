use crate::{data_packer::DataPacker, data_reader::DataReader};

pub struct Transporter {
    pub reader: DataReader,
    pub packer: DataPacker,
}

impl Transporter {
    pub async fn new(uri: &str, user: &str, password: &str, db: &str) -> Self {
        Self {
            reader: DataReader::new(uri, user, password, db).await.unwrap(),
            packer: DataPacker::new().await,
        }
    }

    pub async fn transport_data(&mut self) -> Result<(), ()> {
        tracing::info!("Start to pack the data");
        let _ = self.reader.get_all_programs().await;
        Ok(())
    }
}
