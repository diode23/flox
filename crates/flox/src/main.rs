use anyhow::Result;
use flox_rust_sdk::environment::build_flox_env;
use log::{debug, info};
use std::env;
use std::fmt::Debug;
use std::process::ExitStatus;

use tokio::process::Command;

mod build;
mod config;
mod utils;
pub static FLOX_SH: &str = env!("FLOX_SH");

mod commands {
    use anyhow::Result;
    use bpaf::Bpaf;
    use flox_rust_sdk::flox::Flox;

    use self::package::PackageArgs;

    #[derive(Bpaf)]
    #[bpaf(options)]
    pub struct FloxArgs {
        verbose: bool,

        debug: bool,

        #[bpaf(external(commands))]
        command: Commands,

        #[bpaf(positional)]
        nix_args: Vec<String>,
    }

    impl FloxArgs {
        /// Initialize the command line by creating an initial FloxBuilder
        pub async fn handle(&self, config: crate::config::Config) -> Result<()> {
            let flox = Flox {
                collect_metrics: config.flox.allow_telemetry.unwrap_or_default(),
                cache_dir: config.flox.cache_dir,
                data_dir: config.flox.data_dir,
                config_dir: config.flox.config_dir,
            };

            match self.command {
                // Commands::Support(ref f) => f.run(self).await?,
                // Commands::Build(ref f) => f.run(&self).await?,
                Commands::Package(ref package) => package.handle(flox).await?,
            }
            Ok(())
        }
    }

    /// Transparent separation of different categories of commands
    #[derive(Bpaf)]
    pub enum Commands {
        Package(#[bpaf(external(package::package_args))] PackageArgs),
    }

    mod package {
        use anyhow::Result;
        use bpaf::Bpaf;
        use flox_rust_sdk::{flox::Flox, nix::command_line::NixCommandLine, prelude::Stability};

        use self::build::BuildArgs;

        #[derive(Bpaf)]
        pub struct PackageArgs {
            stability: Option<Stability>,

            #[bpaf(external(package_commands))]
            command: PackageCommands,
        }

        impl PackageArgs {
            pub async fn handle(&self, flox: Flox) -> Result<()> {
                match &self.command {
                    PackageCommands::Build(BuildArgs { installable }) => {
                        flox.package(
                            installable.clone().into(),
                            self.stability.clone().unwrap_or_default(),
                        )
                        .build::<NixCommandLine>()
                        .await?
                    }
                }

                Ok(())
            }
        }

        #[derive(Bpaf)]
        pub enum PackageCommands {
            #[bpaf(command)]
            Build(#[bpaf(external(build::build_args))] build::BuildArgs),
        }

        mod build {
            use bpaf::Bpaf;
            #[derive(Bpaf)]
            pub struct BuildArgs {
                #[bpaf(positional("INSTALLABLE"))]
                pub installable: String,
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    if crate::config::Config::preview_enabled()? {
        run_rust_flox().await?;
    } else {
        info!("`FLOX_PREVIEW` unset or not \"true\", falling back to legacy flox");
        run_in_flox(&env::args_os().collect::<Vec<_>>()[1..]).await?;
    }

    Ok(())
}

async fn run_rust_flox() -> Result<()> {
    let args = commands::flox_args().run();
    args.handle(config::Config::parse()?).await?;
    Ok(())
}

pub async fn run_in_flox(args: &[impl AsRef<std::ffi::OsStr> + Debug]) -> Result<ExitStatus> {
    debug!("Running in flox with arguments: {:?}", args);
    let status = Command::new(FLOX_SH)
        .args(args)
        .envs(&build_flox_env())
        .spawn()
        .expect("failed to spawn flox")
        .wait()
        .await?;

    Ok(status)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_flox_help() {
        // TODO check the output
        assert_eq!(run_in_flox(&["--help"]).await.unwrap().code().unwrap(), 0,)
    }
}