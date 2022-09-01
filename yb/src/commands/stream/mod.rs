use enum_dispatch::enum_dispatch;

pub use add::StreamAddCommand;
pub use list::StreamListCommand;
pub use update::StreamUpdateCommand;

mod add;
mod list;
mod update;

#[enum_dispatch(SubcommandRunner)]
#[derive(Debug, clap::Subcommand)]
pub enum StreamSubcommands {
    Add(StreamAddCommand),
    List(StreamListCommand),
    Update(StreamUpdateCommand),
}
