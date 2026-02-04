use std::collections::HashMap;
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
    /// does testing things
    Test {
        /// lists test values
        #[arg(short, long)]
        list: bool,
    },

    /// does running things
    Run {
        #[arg(short, long)]
        jump: bool,
    },

    /// perform operations on the pin objects
    Pin {
        #[command(subcommand)]
        command: PinCommands,
    },
}

#[derive(Subcommand)]
enum PinCommands {
    /// set the focus on the current screen to the specified tag
    Focus {
        /// the name of the pin on which to focus
        name: String,
    },
}

fn test(list: bool) {
    if list {
        println!("Printing testing lists...");
    } else {
        println!("Not printing testing lists...");
    }
}

fn run(jump: bool) {
    if jump {
        println!("Jumping...");
    } else {
        println!("Not jumping...");
    }
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

    let pins = HashMap::from([
        ("0".to_string(), "0.open".to_string()),
        ("1".to_string(), "1.open".to_string()),
        ("2".to_string(), "2.open".to_string()),
        ("3".to_string(), "3.open".to_string()),
        ("4".to_string(), "4.open".to_string()),
        ("5".to_string(), "5.open".to_string()),
        ("6".to_string(), "6.open".to_string()),
        ("7".to_string(), "7.open".to_string()),
        ("8".to_string(), "8.open".to_string()),
        ("9".to_string(), "9.open".to_string()),
        ("g".to_string(), "1.admin".to_string()),
        ("f".to_string(), "4.dev".to_string()),
        ("d".to_string(), "7.ref".to_string()),
        ("s".to_string(), "3.ai".to_string()),
        ("a".to_string(), "8.chat".to_string()),
    ]);

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
    let repo = Repository::new(conn);
    let _ = repo;
    // let mut project = Project::new("Project 1")
    //     .add_new_view("1", "View 1")
    //     .add_new_view("2", "View 2");
    // model.add_project(project);
    //
    // project = Project::new("Project 2");
    // project.add_view(View::new("3", "View 3"));
    // project.add_view(View::new("4", "View 4"));
    // model.add_project(project.clone());
    //
    // model.set_active_project(project);
    //
    // let v = model.find_view("Not Found", "Tag 1");

    // You can check for the existence of subcommands, and if found use their
    // matches just as you would the top level cmd
    match &cli.command {
        Some(Commands::Test { list }) => {
            test(*list);
        }
        Some(Commands::Run { jump }) => {
            run(*jump);
        }
        Some(Commands::Pin { command }) => match command {
            PinCommands::Focus { name } => {
                let workspace = pins.get(name).expect("No such pin");
                i3.focus(&workspace);
            }
        },

        None => {}
    }

    // Continued program logic goes here...
}
