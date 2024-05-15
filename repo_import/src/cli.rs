use structopt::StructOpt;

#[derive(StructOpt, Debug, Default, Clone)]
pub(crate) struct Cli {
    #[structopt(subcommand)]
    pub(crate) command: Command,

    #[structopt(short, long, required = true)]
    pub(crate) mega_base: String,

    #[structopt(short, long)]
    pub(crate) dont_clone: bool,
}

#[derive(StructOpt, Debug, Default, Clone)]
pub(crate) enum Command {
    #[default]
    Mega,
}
