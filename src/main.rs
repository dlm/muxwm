use std::collections::HashSet;
use std::path::PathBuf;

use anyhow::{Context, Result};
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

    /// list all views for the current active project
    ListViews {},
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
    fn new() -> Result<Self> {
        Ok(Self {
            connection: I3Connection::connect().context("connecting to i3")?,
        })
    }

    fn focus(&mut self, workspace: &str) -> Result<()> {
        let cmd = format!("workspace {}", workspace);
        self.connection
            .run_command(&cmd)
            .with_context(|| format!("running `workspace` command with {}", workspace))?;
        Ok(())
    }

    fn get_active_workspace_name(&mut self) -> Result<String> {
        let result = self.connection.get_workspaces()?;
        result
            .workspaces
            .iter()
            .find(|w| w.focused)
            .map(|w| w.name.clone())
            .ok_or(anyhow::anyhow!("no active workspace"))
    }

    fn get_workspace_names(&mut self) -> Result<Vec<String>> {
        let result = self
            .connection
            .get_workspaces()
            .context("getting workspaces")?;
        Ok(result.workspaces.iter().map(|w| w.name.clone()).collect())
    }

    fn rename_workspace(&mut self, old_name: &str, new_name: &str) -> Result<()> {
        let cmd = format!("rename workspace \"{}\" to \"{}\"", old_name, new_name);
        self.connection
            .run_command(&cmd)
            .context("renameing workspace")?;
        Ok(())
    }
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let mut i3 = WindowManager::new().context("creating window manager")?;

    // Check how many times the debug flag occurred for verbosity
    match cli.debug {
        0 => eprintln!("Debug mode is off"),
        1 => eprintln!("Debug mode is kind of on"),
        2 => eprintln!("Debug mode is on"),
        _ => eprintln!("Don't be crazy"),
    }

    let home = std::env::var("HOME").context("getting $HOME")?;
    let db_path = PathBuf::from(home).join(".local/share/muxwm/muxwm.db");
    let conn = Connection::open(&db_path)
        .with_context(|| format!("opening database at {}", db_path.display()))?;
    let mut repo = Repository::new(conn).context("creating repository")?;

    match &cli.command {
        Commands::Pin { command } => match command {
            PinCommands::Focus { key } => {
                let view = repo
                    .get_view_for_pin_key(key)
                    .context("getting view for pin key")?
                    .ok_or_else(|| anyhow::anyhow!("no view found for pin key '{}'", key))?;
                let display_name = repo
                    .get_window_manager_display_name(&view)
                    .context("getting display name for view")?;
                i3.focus(&display_name)
                    .with_context(|| format!("focusing on workspace '{}'", display_name))?;
            }

            PinCommands::Set { key, project } => {
                let name = i3
                    .get_active_workspace_name()
                    .context("getting active workspace")?;
                if *project {
                    let proj = repo
                        .get_project_from_window_manager_display_name(&name)?
                        .ok_or_else(|| {
                            anyhow::anyhow!("no project found for display name '{}'", name)
                        })?;
                    repo.upsert_pin_for_project(key, &proj)
                        .with_context(|| format!("upserting pin for project '{}'", proj.name()))?;
                } else {
                    let view = repo
                        .get_view_from_window_manager_display_name(&name)?
                        .ok_or_else(|| {
                            anyhow::anyhow!("no view found for display name '{}'", name)
                        })?;
                    repo.upsert_pin_for_view(key, &view)
                        .with_context(|| format!("upserting pin '{}' for view '{}'", key, name))?;
                }
            }

            PinCommands::Clear { key } => {
                repo.clear_pin(&key)
                    .with_context(|| format!("clearing pin '{}'", key))?;
            }

            PinCommands::List {} => {
                let pins = repo.list_pins().context("listing pins")?;
                for pin in pins {
                    let k = pin.key();
                    let view = repo
                        .get_view_for_pin_key(&k)
                        .with_context(|| format!("getting view for pin '{}'", k))?
                        .ok_or_else(|| anyhow::anyhow!("no view found for pin key '{}'", k))?;

                    let view_name = repo
                        .get_window_manager_display_name(&view)
                        .context("getting display name for view")?;
                    println!("{}\t{}\t{}", pin.key(), pin.pin_type(), view_name);
                }
            }
        },

        Commands::Project { command } => match command {
            ProjectCommands::Add { name } => {
                repo.create_project(&name)
                    .with_context(|| format!("creating project '{}'", name))?;
            }

            ProjectCommands::List { with_pins } => {
                let projects = repo.list_projects().context("listing projects")?;
                for proj in projects {
                    let mut pin_key = String::default();
                    if *with_pins {
                        pin_key = repo
                            .get_pin_key_for_project(&proj)
                            .context("getting pin key")?
                            .unwrap_or_default();
                    }

                    println!("{}\t{}", proj.name(), pin_key);
                }
            }

            ProjectCommands::Focus { name } => {
                let proj = repo
                    .get_project_by_name(&name)
                    .context("getting project")?
                    .ok_or_else(|| anyhow::anyhow!("no project found for name '{}'", name))?;
                let view = repo
                    .get_active_view_for_project(&proj)
                    .context("getting active view for project")?;
                let display_name = repo
                    .get_window_manager_display_name(&view)
                    .context("getting display name for active view for project")?;
                i3.focus(&display_name)
                    .with_context(|| format!("focusing on workspace '{}'", display_name))?;
            }

            ProjectCommands::ActivateNextView {} => {
                let current_workspace = i3
                    .get_active_workspace_name()
                    .context("getting active workspace")?;
                let proj = repo
                    .get_project_from_window_manager_display_name(&current_workspace)?
                    .ok_or_else(|| {
                        anyhow::anyhow!("no project found for display name '{}'", current_workspace)
                    })?;
                let next = repo
                    .get_next_view_for_project(&proj)
                    .with_context(|| format!("getting next view for project '{}'", proj.name()))?;
                repo.set_active_view_for_project(&proj, &next)
                    .with_context(|| format!("setting view for project '{}'", proj.name()))?;
                let next_workspace =
                    repo.get_window_manager_display_name(&next)
                        .with_context(|| {
                            format!(
                                "getting display name for project '{}' view '{}'",
                                proj.name(),
                                next.name()
                            )
                        })?;
                i3.focus(&next_workspace)
                    .with_context(|| format!("focusing on workspace '{}'", next_workspace))?;
            }

            ProjectCommands::ActivatePrevView {} => {
                let current_workspace = i3
                    .get_active_workspace_name()
                    .context("getting active workspace")?;
                let proj = repo
                    .get_project_from_window_manager_display_name(&current_workspace)?
                    .ok_or_else(|| {
                        anyhow::anyhow!("no project found for display name '{}'", current_workspace)
                    })?;
                let prev = repo
                    .get_prev_view_for_project(&proj)
                    .with_context(|| format!("getting prev view for project '{}'", proj.name()))?;
                repo.set_active_view_for_project(&proj, &prev)
                    .with_context(|| format!("setting view for project '{}'", proj.name()))?;
                let previous_workspace =
                    repo.get_window_manager_display_name(&prev)
                        .with_context(|| {
                            format!(
                                "getting display name for project '{}' view '{}'",
                                proj.name(),
                                prev.name()
                            )
                        })?;
                i3.focus(&previous_workspace)
                    .with_context(|| format!("focusing on workspace '{}'", previous_workspace))?;
            }

            ProjectCommands::AddView { view_name } => {
                let display_name = i3
                    .get_active_workspace_name()
                    .context("getting active workspace")?;
                let proj = repo
                    .get_project_from_window_manager_display_name(&display_name)?
                    .ok_or_else(|| {
                        anyhow::anyhow!("no project found for display name '{}'", display_name)
                    })?;
                repo.create_view_in_project(&proj, &view_name)
                    .with_context(|| format!("creating view for project '{}'", proj.name()))?;
            }

            ProjectCommands::ListViews {} => {
                let current_workspace = i3
                    .get_active_workspace_name()
                    .context("getting active workspace")?;
                let proj = repo
                    .get_project_from_window_manager_display_name(&current_workspace)?
                    .ok_or_else(|| {
                        anyhow::anyhow!("no project found for display name '{}'", current_workspace)
                    })?;
                let views = repo
                    .list_views_for_project(&proj)
                    .with_context(|| format!("listing views for project '{}'", proj.name()))?;
                for view in views {
                    let display_name =
                        repo.get_window_manager_display_name(&view)
                            .with_context(|| {
                                format!(
                                    "getting display name for project '{}' view '{}'",
                                    proj.name(),
                                    view.name()
                                )
                            })?;
                    println!("{}", display_name);
                }
            }
        },

        Commands::View { command } => match command {
            ViewCommands::Rename { new_name } => {
                let old_display_name = i3
                    .get_active_workspace_name()
                    .context("getting active workspace")?;
                let view = repo
                    .get_view_from_window_manager_display_name(&old_display_name)?
                    .ok_or_else(|| {
                        anyhow::anyhow!("no view found for display name '{}'", old_display_name)
                    })?;
                let updated_view = repo.rename_view(&view, &new_name).with_context(|| {
                    format!("renaming view '{}' to '{}'", view.name(), new_name)
                })?;
                let new_display_name = repo
                    .get_window_manager_display_name(&updated_view)
                    .with_context(|| {
                        format!(
                            "getting display name for view '{}' after renaming",
                            updated_view.name()
                        )
                    })?;
                i3.rename_workspace(&old_display_name, &new_display_name)
                    .with_context(|| {
                        format!(
                            "renaming workspace '{}' to '{}'",
                            old_display_name, new_display_name
                        )
                    })?;
            }

            ViewCommands::List {
                with_pins,
                with_unmanaged,
            } => {
                let view_names = repo
                    .list_views()
                    .context("listing views")?
                    .iter()
                    .map(|v| {
                        repo.get_window_manager_display_name(v).with_context(|| {
                            format!("getting display name for view '{}'", v.name())
                        })
                    })
                    .collect::<Result<Vec<String>, _>>()?;

                let mut unique_names = if *with_unmanaged {
                    let i3_view_names = i3.get_workspace_names().context("getting workspaces")?;
                    HashSet::<String>::from_iter(view_names.into_iter().chain(i3_view_names))
                        .into_iter()
                        .collect::<Vec<String>>()
                } else {
                    view_names
                };

                unique_names.sort();

                for name in &unique_names {
                    let pin_key = if *with_pins {
                        match repo.get_view_from_window_manager_display_name(name)? {
                            Some(view) => repo
                                .get_pin_key_for_view(&view)
                                .context("getting pin key for view")?
                                .unwrap_or_default(),
                            None => String::default(),
                        }
                    } else {
                        String::default()
                    };

                    println!("{}\t{}", name, pin_key);
                }
            }
        },
    }
    Ok(())
}
