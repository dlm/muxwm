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
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// perform operations on the pin objects
    Pin {
        #[command(subcommand)]
        command: PinCommands,
    },

    /// perform operations on the project objects
    Project {
        #[command(subcommand)]
        command: ProjectCommands,
    },

    /// perform operations on the view objects
    View {
        #[command(subcommand)]
        command: ViewCommands,
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
    List {
        /// show the pins for each project
        /// (default: false)
        #[arg(long)]
        with_pins: bool,
    },

    /// focus on the project's active view
    Focus {
        /// the name of the project
        name: String,
    },

    /// update the current active project's active view to the next view
    /// in the view list
    ActivateNextView {},

    /// update the current active project's active view to the previous view
    /// in the view list
    ActivatePrevView {},
}

#[derive(Subcommand)]
enum ViewCommands {
    /// list all views
    List {
        /// show the pins for each view
        /// (default: false)
        #[arg(long)]
        with_pins: bool,

        /// show all the views, even those that are managed by muxwm, for example, in i3, that
        /// would be all the workspaces managed by the window manager
        ///
        /// (default: false)
        #[arg(long)]
        with_unmanaged: bool,
    },

    /// add a new view
    Add {
        /// the name of the project
        project_name: String,

        /// the name of the view
        view_name: String,
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

    fn get_workspace_names(&mut self) -> Vec<String> {
        let result = self
            .connection
            .get_workspaces()
            .expect("Failed to run `workspace` command");
        result.workspaces.iter().map(|w| w.name.clone()).collect()
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
        Commands::Pin { command } => match command {
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

        Commands::Project { command } => match command {
            ProjectCommands::Add { name } => {
                let id = repo.add_project(&name).unwrap();
                let proj = repo.get_project_by_id(id).unwrap();
                let view = repo.get_active_view_for_project(&proj).unwrap();
                let display_name = repo.get_window_manager_display_name(&view).unwrap();
                i3.focus(&display_name);
            }

            ProjectCommands::List { with_pins } => {
                let projects = repo.list_projects().unwrap();
                for proj in projects {
                    let mut pin_key = String::new();
                    if *with_pins {
                        let active_view = repo.get_active_view_for_project(&proj).unwrap();
                        pin_key = repo
                            .get_pin_key_for_view(&active_view)
                            .unwrap_or("".to_string());
                    }

                    println!("{}\t{}", proj.name(), pin_key);
                }
            }

            ProjectCommands::Focus { name } => {
                let proj = repo.get_project_by_name(&name).unwrap();
                let view = repo.get_active_view_for_project(&proj).unwrap();
                let display_name = repo.get_window_manager_display_name(&view).unwrap();
                i3.focus(&display_name);
            }

            ProjectCommands::ActivateNextView {} => {
                let display_name = i3.get_active_workspace_name().unwrap();
                let proj = repo
                    .get_project_from_window_manager_display_name(&display_name)
                    .unwrap();
                let next = repo.get_next_view_for_project(&proj).unwrap();
                let _ = repo.set_active_view_for_project(&proj, &next).unwrap();
                let display_name = repo.get_window_manager_display_name(&next).unwrap();
                i3.focus(&display_name);
            }

            ProjectCommands::ActivatePrevView {} => {
                let display_name = i3.get_active_workspace_name().unwrap();
                let proj = repo
                    .get_project_from_window_manager_display_name(&display_name)
                    .unwrap();
                let prev = repo.get_prev_view_for_project(&proj).unwrap();
                let _ = repo.set_active_view_for_project(&proj, &prev).unwrap();
                let display_name = repo.get_window_manager_display_name(&prev).unwrap();
                i3.focus(&display_name);
            }
        },

        Commands::View { command } => match command {
            ViewCommands::List {
                with_pins,
                with_unmanaged,
            } => {
                if !with_unmanaged {
                    eprintln!("printing only the managed workspaces is not yet supported");
                    std::process::exit(1);
                }

                // BUGBUG: we are missing the views that are managed by mux but not in i3

                let view_names = i3.get_workspace_names();
                view_names.iter().for_each(|name| {
                    let pin_key = if *with_pins {
                        repo.get_view_from_window_manager_display_name(name)
                            .and_then(|view| repo.get_pin_key_for_view(&view))
                            .unwrap_or("".to_string())
                    } else {
                        String::new()
                    };

                    println!("{}\t{}", name, pin_key);
                });
            }

            ViewCommands::Add {
                project_name,
                view_name,
            } => {
                let proj = repo.get_project_by_name(&project_name).unwrap();
                let view = repo.add_view_to_project(&proj, &view_name).unwrap();
                let display_name = repo.get_window_manager_display_name(&view).unwrap();
                println!("added view: {}", display_name);
            }
        },
    }
}
