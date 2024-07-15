//! the core controller module
//! It receives messages from Kafka MQ one by one,
//! parse them, and store it into tugraph, and notify
//! other processes.

use crate::cli::CratesProCli;

pub struct CoreController<'a> {
    pub cli: &'a CratesProCli,
}

impl CoreController<'_> {
    pub async fn run(&self) {
        repo_import::repo_main(self.cli.dont_clone, &self.cli.mega_base).await;
    }
}
