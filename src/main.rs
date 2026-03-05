use std::collections::HashSet;
use std::path::PathBuf;

use anyhow::Result;
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

        /// if true, the pin will be a project pin
        #[arg(long)]
        project: bool,
    },

    /// clear the pin
    Clear {
        /// the pin key of the view on which to focus
        key: String,
    },

    /// list all pins
    List {},
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
    /// in the view list and focus it
    ActivateNextView {},

    /// update the current active project's active view to the previous view
    /// in the view list and focus it
    ActivatePrevView {},

    /// add a new view to the current active project
    AddView {
        /// the name of the view
        view_name: String,
    },
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

    // rename the currently active view
    Rename {
        /// the new name of the view
        new_name: String,
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
            .expect("Failed to run `get_workspaces` command");
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
            .expect("Failed to run `get_workspace` command");
        result.workspaces.iter().map(|w| w.name.clone()).collect()
    }

    fn rename_workspace(&mut self, old_name: &str, new_name: &str) -> Result<()> {
        let cmd = format!("rename workspace \"{}\" to \"{}\"", old_name, new_name);
        self.connection.run_command(&cmd)?;
        Ok(())
    }
}

fn main() {
    let cli = Cli::parse();
    let mut i3 = WindowManager::new();

    // Check how many times the debug flag occurred for verbosity
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

            PinCommands::Set { key, project } => {
                let name = i3.get_active_workspace_name().unwrap();
                if *project {
                    let proj = repo
                        .get_project_from_window_manager_display_name(&name)
                        .unwrap()
                        .unwrap();
                    repo.upsert_pin_for_project(key, &proj).unwrap();
                } else {
                    let view = repo
                        .get_view_from_window_manager_display_name(&name)
                        .unwrap()
                        .unwrap();
                    repo.upsert_pin_for_view(key, &view).unwrap();
                }
            }

            PinCommands::Clear { key } => {
                repo.clear_pin(&key).unwrap();
            }

            PinCommands::List {} => {
                let pins = repo.list_pins().unwrap();
                for pin in pins {
                    let view = repo.get_view_for_pin_key(&pin.key()).unwrap();
                    let view_name = repo.get_window_manager_display_name(&view).unwrap();
                    println!("{}\t{}\t{}", pin.key(), pin.pin_type(), view_name);
                }
            }
        },

        Commands::Project { command } => match command {
            ProjectCommands::Add { name } => {
                let proj = repo.create_project(&name).unwrap();
                let view = repo.get_active_view_for_project(&proj).unwrap();
                let display_name = repo.get_window_manager_display_name(&view).unwrap();
                println!("added project: {}", display_name);
            }

            ProjectCommands::List { with_pins } => {
                let projects = repo.list_projects().unwrap();
                for proj in projects {
                    let mut pin_key = String::new();
                    if *with_pins {
                        pin_key = repo
                            .get_pin_key_for_project(&proj)
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
                    .unwrap()
                    .unwrap();
                let next = repo.get_next_view_for_project(&proj).unwrap();
                repo.set_active_view_for_project(&proj, &next).unwrap();
                let display_name = repo.get_window_manager_display_name(&next).unwrap();
                i3.focus(&display_name);
            }

            ProjectCommands::ActivatePrevView {} => {
                let display_name = i3.get_active_workspace_name().unwrap();
                let proj = repo
                    .get_project_from_window_manager_display_name(&display_name)
                    .unwrap()
                    .unwrap();
                let prev = repo.get_prev_view_for_project(&proj).unwrap();
                let _ = repo.set_active_view_for_project(&proj, &prev).unwrap();
                let display_name = repo.get_window_manager_display_name(&prev).unwrap();
                i3.focus(&display_name);
            }

            ProjectCommands::AddView { view_name } => {
                let display_name = i3.get_active_workspace_name().unwrap();
                let proj = repo
                    .get_project_from_window_manager_display_name(&display_name)
                    .unwrap()
                    .unwrap();
                let view = repo.create_view_in_project(&proj, &view_name).unwrap();
                let display_name = repo.get_window_manager_display_name(&view).unwrap();
                println!("added view: {}", display_name);
            }
        },

        Commands::View { command } => match command {
            ViewCommands::Rename { new_name } => {
                let old_display_name = i3.get_active_workspace_name().unwrap();
                let view = repo
                    .get_view_from_window_manager_display_name(&old_display_name)
                    .unwrap()
                    .unwrap();
                let updated_view = repo.rename_view(&view, &new_name).unwrap();
                let new_display_name = repo.get_window_manager_display_name(&updated_view).unwrap();
                let _ = i3
                    .rename_workspace(&old_display_name, &new_display_name)
                    .unwrap();
            }

            ViewCommands::List {
                with_pins,
                with_unmanaged,
            } => {
                let view_names = repo
                    .list_views()
                    .unwrap()
                    .iter()
                    .map(|v| repo.get_window_manager_display_name(v).unwrap())
                    .collect::<Vec<String>>();

                let mut unique_names = if *with_unmanaged {
                    let i3_view_names = i3.get_workspace_names();
                    HashSet::<String>::from_iter(view_names.into_iter().chain(i3_view_names))
                        .into_iter()
                        .collect::<Vec<String>>()
                } else {
                    view_names
                };

                unique_names.sort();

                for name in &unique_names {
                    let pin_key = if *with_pins {
                        repo.get_view_from_window_manager_display_name(name)
                            .unwrap()
                            .and_then(|view| repo.get_pin_key_for_view(&view))
                            .unwrap_or_default()
                    } else {
                        String::new()
                    };

                    println!("{}\t{}", name, pin_key);
                }
            }
        },
    }
}
