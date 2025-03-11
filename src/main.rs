// Use `cargo clippy -- -W clippy::pedantic -W clippy::nursery` to get more tips on your code
// Use `///` if you want to document a function, struct, or struct member
use clap::Parser;
use leon::Template;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

const DBNAME: &str = "cmdwrap.json";
const PROGNAME: &str = "cmdwrap";

/// CLAP
#[derive(Parser, Debug)]
struct Args {
    #[arg(
        short,
        long,
        default_value_t = false,
        help = "Dump default config on stdout"
    )]
    dump_config: bool,

    #[arg(short, long, help = "Create symlinks in path")]
    create_symlink: Option<PathBuf>,
}

#[derive(Serialize, Deserialize)]
struct Settings {
    commands: Vec<Cmd>,
}

#[derive(Serialize, Deserialize)]
struct Cmd {
    name: String,
    image: String,
    command: Option<String>,
    docker_args: String,
}

impl Settings {
    fn lookup_progam(&self, prog_name: &str) -> Option<&Cmd> {
        self.commands.iter().find(|entry| entry.name == prog_name)
    }

    fn from_path(path: &PathBuf) -> Option<Self> {
        let data = fs::read_to_string(path).expect("Unable to read database file");
        Settings::from_str(&data)
    }

    fn from_str(s: &str) -> Option<Self> {
        let setting: Settings = serde_json::from_str(s).expect("Problems");
        Some(setting)
    }

    fn default() -> Self {
        let bytes = include_bytes!("../config/cmdwrap.json");
        let settings: Settings =
            serde_json::from_str(std::str::from_utf8(bytes).unwrap()).expect("Problems");
        settings
    }
}

fn find_config() -> Option<PathBuf> {
    let mut current_dir = match std::env::current_dir() {
        Ok(dir) => dir,
        Err(_) => return None,
    };

    //first go up in the directory tree
    loop {
        let file_path = current_dir.join(DBNAME);
        //println!("Search {file_path:?}");
        if file_path.exists() {
            return Some(current_dir);
        }
        if !current_dir.pop() {
            break;
        }
    }
    // look in ~/.config/cmdwarp/cmdrap.json
    if let Some(dir) = dirs::config_dir() {
        let file_path = dir.join(PROGNAME).join(DBNAME);
        //println!("Search {file_path:?}");
        if file_path.exists() {
            return Some(file_path);
        }
    }
    // look in /etc/cmdwrap/cmdwrap.json
    let file_path = PathBuf::from("/etc/").join(PROGNAME).join(DBNAME);
    //println!("Search {file_path:?}");
    if file_path.exists() {
        return Some(file_path);
    }
    None
}

/// Attempt to find the top directory of the project by looking for a file named `DBNAME` else
/// return the home directory
fn find_top() -> Option<PathBuf> {
    let mut current_dir = match std::env::current_dir() {
        Ok(dir) => dir,
        Err(_) => return None,
    };

    loop {
        let file_path = current_dir.join(DBNAME);
        if file_path.exists() {
            return Some(current_dir);
        }
        if !current_dir.pop() {
            break;
        }
    }
    dirs::home_dir()
}

#[derive(Debug, Clone)]
struct DatabaseFindError;

impl fmt::Display for DatabaseFindError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Failed to find database")
    }
}

fn find_database() -> Result<PathBuf, DatabaseFindError> {
    find_config().ok_or(DatabaseFindError)
}

// Wether the command invoked is not a symlink e.g.
// cmdwrap itself
fn is_self() -> bool {
    let args = std::env::args().collect::<Vec<String>>();
    let link_data = fs::symlink_metadata(&args[0]);
    //if the file is not a symbolic link we assume we are the main
    !link_data.expect("WFT").file_type().is_symlink()
}

fn create_symlinks(target_dir: PathBuf, settings: &Settings) {
    println!("Creating symbolic links");
    if target_dir.is_dir() {
        println!("Cool this is a dir");
        for cmd in &settings.commands {
            println!("Create symlink {}", cmd.name);
        }
    } else {
        println!("Sorry target {:?} is not a directory", target_dir);
    }
}

/// Starts a Docker container and run a command inside it.
fn run_command_in_container(image: &str, docker_args: Vec<&str>, cmd: &str, args: Vec<&str>) {
    //println!("[{:?}]", docker_args);
    let mut child = Command::new("docker")
        .arg("run")
        .arg("-it")
        .args(docker_args)
        .arg(image)
        .arg(cmd)
        .args(args)
        .spawn()
        .expect("Unable to start Docker container");
    child.wait().unwrap();
}

// The main entry point of the wrapper.
fn main() {
    //if we are not called from a symlink behave differently
    if is_self() {
        let args = Args::parse();
        if args.dump_config {
            let settings = Settings::default();
            if let Ok(a) = serde_json::to_string_pretty(&settings) {
                println!("{}", a);
            }
            return;
        }
    }

    // Read the JSON database from disk.
    let db_path = find_database().expect("Can not find database");
    let settings = Settings::from_path(&db_path).expect("database reading problem");

    if is_self() {
        // a second self section after reading the settings
        println!("Reading from {:?}", &db_path);
        let args = Args::parse();
        if let Some(target_dir) = args.create_symlink {
            //check if the given path exist and is a directory
            create_symlinks(target_dir, &settings);
        }
        return;
    }
    // For more complicated arg parsing, clap is a very nice library
    let args = std::env::args().collect::<Vec<String>>();

    let progname = &args[0];

    // strip the potential path name from the exeutable name and convert to string
    let progname = Path::new(progname).file_name().unwrap().to_string_lossy();

    // Look up the Docker container information for the given binary name.
    let container = settings.lookup_progam(&progname);

    // If a matching container was found, start it and run the command.
    if let Some(container) = container {
        let image = container.image.as_str();
        let command = match &container.command {
            Some(command) => command.as_str(),
            None => container.name.as_str(),
        };

        let mut values = HashMap::new();

        let top_str = find_top().unwrap().to_string_lossy().into_owned();
        let pwd_str = std::env::current_dir()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        values.insert("top", &top_str);
        values.insert("pwd", &pwd_str);

        let template = Template::parse(container.docker_args.as_str()).unwrap();
        let docker_args = template.render(&values);

        let str_args: Vec<&str> = args.iter().skip(1).map(|item| item as &str).collect();
        run_command_in_container(
            image,
            docker_args
                .unwrap()
                .as_str()
                .split(' ')
                .collect::<Vec<&str>>(),
            command,
            str_args,
        );
    } else {
        // If no matching container was found, print an error message and exit.
        eprintln!("No container found for binary '{}'.", progname);
        std::process::exit(1);
    }
}
