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

    Project {
        #[command(subcommand)]
        command: ProjectCommands,
    },
}

#[derive(Subcommand)]
enum PinCommands {
    /// set the focus on the current screen to the specified view
    Focus {
        /// the pin key of the view on which to focus
        key: String,
    },

    /// set the pin to the currently focused view
    Set {
        /// the pin key of the view on which to focus
        key: String,
    },

    /// clear the pin
    Clear {
        /// the pin key of the view on which to focus
        key: String,
    },
}

#[derive(Subcommand)]
enum ProjectCommands {
    /// add a new project
    Add {
        /// the name of the project
        name: String,
    },

    /// list all projects
    List,

    /// focus on the project's active view
    Focus {
        /// the name of the project
        name: String,
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

    fn get_active_workspace_name(&mut self) -> Option<String> {
        let result = self
            .connection
            .get_workspaces()
            .expect("Failed to run `workspace` command");
        result
            .workspaces
            .iter()
            .find(|w| w.focused)
            .map(|w| w.name.clone())
    }
}

fn main() {
    let cli = Cli::parse();
    let mut i3 = WindowManager::new();

    // You can check the value provided by positional arguments, or option arguments
    if let Some(name) = cli.name.as_deref() {
        eprintln!("Value for name: {name}");
    }

    if let Some(config_path) = cli.config.as_deref() {
        eprintln!("Value for config: {}", config_path.display());
    }

    // You can see how many times a particular flag or argument occurred
    // Note, only flags can have multiple occurrences
    match cli.debug {
        0 => eprintln!("Debug mode is off"),
        1 => eprintln!("Debug mode is kind of on"),
        2 => eprintln!("Debug mode is on"),
        _ => eprintln!("Don't be crazy"),
    }

    let home = std::env::var("HOME").unwrap();
    let db_path = PathBuf::from(home).join(".local/share/muxwm/muxwm.db");
    let conn = Connection::open(db_path).unwrap();
    let mut repo = Repository::new(conn).unwrap();

    match &cli.command {
        Some(Commands::Pin { command }) => match command {
            PinCommands::Focus { key } => {
                let view = repo.get_view_for_pin_key(key).unwrap();
                let display_name = repo.get_window_manager_display_name(&view).unwrap();
                i3.focus(&display_name);
            }

            PinCommands::Set { key } => {
                let name = i3.get_active_workspace_name().unwrap();
                let view = repo
                    .get_view_from_window_manager_display_name(&name)
                    .unwrap();
                repo.upsert_pin(key, &view).unwrap();
            }

            PinCommands::Clear { key } => {
                repo.clear_pin(&key).unwrap();
            }
        },

        Some(Commands::Project { command }) => match command {
            ProjectCommands::Add { name } => {
                let id = repo.add_project(&name).unwrap();
                let proj = repo.get_project_by_id(id).unwrap();
                let view = repo.get_active_view_for_project(&proj).unwrap();
                let display_name = repo.get_window_manager_display_name(&view).unwrap();
                i3.focus(&display_name);
            }

            ProjectCommands::List => {
                let projects = repo.list_projects().unwrap();
                for proj in projects {
                    println!("{}", proj.name());
                }
            }

            ProjectCommands::Focus { name } => {
                let proj = repo.get_project_by_name(&name).unwrap();
                let view = repo.get_active_view_for_project(&proj).unwrap();
                let display_name = repo.get_window_manager_display_name(&view).unwrap();
                i3.focus(&display_name);
            }
        },

        None => {}
    }
}
