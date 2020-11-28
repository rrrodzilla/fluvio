//!
//! # Root CLI
//!
//! CLI configurations at the top of the tree

use std::sync::Arc;
use structopt::clap::{AppSettings, Shell};
use structopt::StructOpt;
use tracing::debug;

use fluvio::Fluvio;
use fluvio_extension_consumer::consume::ConsumeLogOpt;
use fluvio_extension_consumer::produce::ProduceLogOpt;
use fluvio_extension_consumer::partition::PartitionCmd;
use fluvio_extension_consumer::topic::TopicCmd;
use fluvio_cluster::extension::ClusterCmd;

use crate::Result;
use crate::common::COMMAND_TEMPLATE;
use crate::common::target::ClusterTarget;
use crate::common::output::Terminal;

use crate::profile::ProfileCmd;
use crate::install::update::UpdateOpt;
use crate::install::plugins::InstallOpt;

#[cfg(any(feature = "cluster_components", feature = "cluster_components_rustls"))]
use super::run::RunOpt;

/// Fluvio Command Line Interface
#[derive(StructOpt, Debug)]
pub struct Root {
    #[structopt(flatten)]
    opts: RootOpt,
    #[structopt(subcommand)]
    command: RootCmd,
}

impl Root {
    pub async fn process(self) -> Result<()> {
        self.command.process(self.opts).await?;
        Ok(())
    }
}

#[derive(StructOpt, Debug)]
struct RootOpt {
    #[structopt(flatten)]
    target: ClusterTarget,
}

#[derive(Debug, StructOpt)]
#[structopt(
    about = "Fluvio Command Line Interface",
    name = "fluvio",
    template = COMMAND_TEMPLATE,
    max_term_width = 80,
    global_settings = &[AppSettings::VersionlessSubcommands, AppSettings::DeriveDisplayOrder, AppSettings::DisableVersion]
)]
enum RootCmd {
    /// All top-level commands that require a Fluvio client are bundled in `FluvioCmd`
    #[structopt(flatten)]
    Fluvio(FluvioCmd),

    /// Manage Profiles, which describe linked clusters
    ///
    /// Each Profile describes a particular Fluvio cluster you may be connected to.
    /// This might correspond to Fluvio running on Minikube or in the Cloud.
    /// There is one "active" profile, which determines which cluster all of the
    /// Fluvio CLI commands interact with.
    #[structopt(name = "profile")]
    Profile(ProfileCmd),

    /// Install or uninstall Fluvio clusters
    ///
    /// If you are not using Fluvio Cloud, you may wish to install your own Fluvio
    /// cluster, running either directly on your computer (local), or hosted inside
    /// of a Minikube kubernetes environment. These cluster commands will help you
    /// to set up these types of installations.
    #[structopt(name = "cluster")]
    Cluster(Box<ClusterCmd>),

    /// Install Fluvio plugins
    ///
    /// The Fluvio CLI considers any executable with the prefix `fluvio-` to be a
    /// CLI plugin. For example, an executable named `fluvio-foo` in your PATH may
    /// be invoked by running `fluvio foo`.
    ///
    /// This command allows you to install plugins from Fluvio's package registry.
    #[structopt(name = "install")]
    Install(InstallOpt),

    /// Update the Fluvio CLI
    #[structopt(name = "update")]
    Update(UpdateOpt),

    /// Print Fluvio version information
    #[structopt(name = "version")]
    Version(VersionOpt),

    /// Generate command-line completions for Fluvio
    #[structopt(
        name = "completions",
        settings = &[AppSettings::Hidden]
    )]
    Completions(CompletionCmd),

    #[structopt(external_subcommand)]
    External(Vec<String>),
}

impl RootCmd {
    pub async fn process(self, root: RootOpt) -> Result<()> {
        let out = Arc::new(PrintTerminal::new());

        match self {
            Self::Fluvio(fluvio_cmd) => {
                fluvio_cmd.process(out, root.target).await?;
            }
            Self::Profile(profile) => {
                profile.process(out).await?;
            }
            Self::Cluster(cluster) => {
                cluster.process(out, root.target).await?;
            }
            Self::Install(install) => {
                install.process().await?;
            }
            Self::Update(update) => {
                update.process().await?;
            }
            Self::Version(version) => {
                version.process()?;
            }
            Self::Completions(completion) => {
                completion.process()?;
            }
            Self::External(args) => {
                process_external_subcommand(args)?;
            }
        }

        Ok(())
    }
}

// For some reason this doc string is the one that gets used for the top-level help menu.
// Please don't change it unless you want to update the top-level help menu "about".
/// Fluvio command-line interface
#[derive(StructOpt, Debug)]
pub enum FluvioCmd {
    /// Read messages from a topic/partition
    ///
    /// By default, this activates in "follow" mode, where
    /// the command will hang and continue to wait for new
    /// messages, printing them as they arrive. You can use
    /// the `-d` switch to consume all available messages,
    /// but exit upon reaching the end of the stream.
    #[structopt(name = "consume")]
    Consume(ConsumeLogOpt),

    /// Write messages to a topic/partition
    ///
    /// By default, this reads a single line from stdin to
    /// use as the message. If you run this with no file options,
    /// the command will hang until you type a line in the terminal.
    /// Alternatively, you can pipe a message into the command
    /// like this:
    ///
    /// $ echo "Hello, world" | fluvio produce greetings
    #[structopt(name = "produce")]
    Produce(ProduceLogOpt),

    /// Manage and view Topics
    ///
    /// A Topic is essentially the name of a stream which carries messages that
    /// are related to each other. Similar to the role of tables in a relational
    /// database, the names and contents of Topics will typically reflect the
    /// structure of the application domain they are used for.
    #[structopt(name = "topic")]
    Topic(TopicCmd),

    /// Manage and view Partitions
    ///
    /// Partitions are a way to divide the total traffic of a single Topic into
    /// separate streams which may be processed independently. Data sent to different
    /// partitions may be processed by separate SPUs on different computers. By
    /// dividing the load of a Topic evenly among partitions, you can increase the
    /// total throughput of the Topic.
    #[structopt(name = "partition")]
    Partition(PartitionCmd),
}

impl FluvioCmd {
    /// Connect to Fluvio and pass the Fluvio client to the subcommand handlers.
    pub async fn process<O: Terminal>(self, out: Arc<O>, target: ClusterTarget) -> Result<()> {
        let fluvio_config = target.load()?;
        let fluvio = Fluvio::connect_with_config(&fluvio_config).await?;

        match self {
            Self::Consume(consume) => {
                consume.process(out, &fluvio).await?;
            }
            Self::Produce(produce) => {
                produce.process(out, &fluvio).await?;
            }
            Self::Topic(topic) => {
                topic.process(out, &fluvio).await?;
            }
            Self::Partition(partition) => {
                partition.process(out, &fluvio).await?;
            }
        }

        Ok(())
    }
}

struct PrintTerminal {}

impl PrintTerminal {
    fn new() -> Self {
        Self {}
    }
}

impl Terminal for PrintTerminal {
    fn print(&self, msg: &str) {
        print!("{}", msg);
    }

    fn println(&self, msg: &str) {
        println!("{}", msg);
    }
}

#[derive(Debug, StructOpt)]
struct VersionOpt {}

impl VersionOpt {
    pub fn process(self) -> Result<()> {
        println!("Fluvio version : {}", crate::VERSION);
        println!("Git Commit     : {}", env!("GIT_HASH"));
        if let Some(os_info) = option_env!("UNAME") {
            println!("OS Details     : {}", os_info);
        }
        println!("Rustc Version  : {}", env!("RUSTC_VERSION"));
        Ok(())
    }
}

#[derive(Debug, StructOpt)]
struct CompletionOpt {
    #[structopt(long, default_value = "fluvio")]
    name: String,
}

#[derive(Debug, StructOpt)]
enum CompletionCmd {
    /// Generate CLI completions for bash
    #[structopt(name = "bash")]
    Bash(CompletionOpt),
    // Zsh generation currently has a bug that causes panic
    // /// Generate CLI completions for zsh
    // #[structopt(name = "zsh")]
    // Zsh(CompletionOpt),
    /// Generate CLI completions for fish
    #[structopt(name = "fish")]
    Fish(CompletionOpt),
}

impl CompletionCmd {
    pub fn process(self) -> Result<()> {
        let mut app: structopt::clap::App = RootCmd::clap();
        match self {
            Self::Bash(opt) => {
                app.gen_completions_to(opt.name, Shell::Bash, &mut std::io::stdout());
            }
            // Self::Zsh(opt) => {
            //     app.gen_completions_to(opt.name, Shell::Zsh, &mut std::io::stdout());
            // }
            Self::Fish(opt) => {
                app.gen_completions_to(opt.name, Shell::Fish, &mut std::io::stdout());
            }
        }
        Ok(())
    }
}

fn process_external_subcommand(mut args: Vec<String>) -> Result<()> {
    use std::fs;
    use std::process::Command;
    use std::path::PathBuf;

    // The external subcommand's name is given as the first argument, take it.
    let cmd = args.remove(0);
    // Check for a matching external command in the environment

    let external_subcommand = format!("fluvio-{}", cmd);
    let mut subcommand_path: Option<PathBuf> = None;

    let fluvio_dir = crate::install::fluvio_extensions_dir()?;

    if let Ok(entries) = fs::read_dir(&fluvio_dir) {
        for entry in entries {
            if let Ok(entry) = entry {
                if entry.path().ends_with(&external_subcommand) {
                    subcommand_path = Some(entry.path());
                    break;
                }
            }
        }
    }
    let subcommand_path = match subcommand_path {
        Some(path) => path,
        None => {
            println!(
                "Unable to find plugin '{}'. Make sure it is installed in {:?}.",
                &external_subcommand, fluvio_dir,
            );
            std::process::exit(1);
        }
    };

    // Print the fully-qualified command to debug
    let args_string = args.join(" ");
    debug!(
        "Launching external subcommand: {} {}",
        subcommand_path.as_path().display(),
        &args_string
    );

    // Execute the command with the provided arguments
    let status = Command::new(subcommand_path.as_path())
        .args(&args)
        .status()?;

    if let Some(code) = status.code() {
        std::process::exit(code);
    }

    Ok(())
}
