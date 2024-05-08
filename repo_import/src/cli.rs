use structopt::StructOpt;

#[derive(StructOpt)]
pub(crate) struct Cli {
    #[structopt(subcommand)]
    pub(crate) command: Command,

    #[structopt(short, long, required = true)]
    pub(crate) mega_base: String,
}

#[derive(StructOpt)]
pub(crate) enum Command {
    Local,
    Mega,
}
