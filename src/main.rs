use std::path::PathBuf;

use clap::{Parser, Subcommand};
use i3ipc::I3Connection;

mod model;
use model::Repository;
use rusqlite::Connection;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    /// Optional name to operate on
    name: Option<String>,

    /// Sets a custom config file
    #[arg(short, long, value_name = "FILE")]
    config: Option<PathBuf>,

    /// Turn debugging information on
    #[arg(short, long, action = clap::ArgAction::Count)]
    debug: u8,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// perform operations on the pin objects
    Pin {
        #[command(subcommand)]
        command: PinCommands,
    },
}

#[derive(Subcommand)]
enum PinCommands {
    /// set the focus on the current screen to the specified view
    Focus {
        /// the pin key of the view on which to focus
        key: String,
    },
}

struct WindowManager {
    connection: I3Connection,
}

impl WindowManager {
    fn new() -> Self {
        Self {
            connection: I3Connection::connect().expect("Failed to connect to i3"),
        }
    }

    fn focus(&mut self, workspace: &str) {
        let cmd = format!("workspace {}", workspace);
        self.connection
            .run_command(&cmd)
            .expect("Failed to run `workspace` command");
    }
}

fn main() {
    let cli = Cli::parse();
    let mut i3 = WindowManager::new();

    // You can check the value provided by positional arguments, or option arguments
    if let Some(name) = cli.name.as_deref() {
        println!("Value for name: {name}");
    }

    if let Some(config_path) = cli.config.as_deref() {
        println!("Value for config: {}", config_path.display());
    }

    // You can see how many times a particular flag or argument occurred
    // Note, only flags can have multiple occurrences
    match cli.debug {
        0 => println!("Debug mode is off"),
        1 => println!("Debug mode is kind of on"),
        2 => println!("Debug mode is on"),
        _ => println!("Don't be crazy"),
    }

    let conn = Connection::open_in_memory().unwrap();
    let mut repo = Repository::new(conn).unwrap();
    repo.add_project("admin").unwrap();
    repo.add_project("dev").unwrap();
    repo.add_project("ref").unwrap();
    repo.add_project("ai").unwrap();
    repo.add_project("chat").unwrap();
    repo.add_project("share").unwrap();

    match &cli.command {
        Some(Commands::Pin { command }) => match command {
            PinCommands::Focus { key } => {
                let view = repo.get_view_for_pin_key(key).unwrap();
                let display_name = repo.get_window_manager_display_name(&view).unwrap();
                i3.focus(&display_name);
            }
        },

        None => {}
    }
}
