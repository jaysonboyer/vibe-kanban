pub mod args;
pub mod compose;
pub mod docker;
pub mod down;
pub mod env;
pub mod error;
pub mod init;
pub mod logs;
pub mod status;
pub mod up;

pub use error::RemoteCliError;

pub async fn run(args: args::RemoteArgs) -> Result<(), RemoteCliError> {
    use args::RemoteCommand;
    match args.command {
        RemoteCommand::Up(a) => up::run(a).await,
        RemoteCommand::Down(a) => down::run(a).await,
        RemoteCommand::Status(a) => status::run(a).await,
        RemoteCommand::Logs(a) => logs::run(a).await,
        RemoteCommand::Init(a) => init::run(a).await,
    }
}
