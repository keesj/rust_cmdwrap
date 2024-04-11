// Generic feedback
// Use `cargo clippy -- -W clippy::pedantic -W clippy::nursery` to get more tips on your code
// Use `///` if you want to document a function, struct, or struct member

use leon::Template;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::path::Path;
use std::process::Command;

const DBNAME: &str = "cmdwrap.json";

#[derive(Serialize, Deserialize)]
struct Settings {
    commands: Vec<Cmd>,
}

#[derive(Serialize, Deserialize)]
struct Cmd {
    name: String,
    image: String,
    command: String,
    docker_args: String,
}

impl Settings {
    fn lookup_progam(&self, prog_name: &str) -> Option<&Cmd> {
        self.commands.iter().find(|entry| entry.name == prog_name)
    }

    fn from_path(path: PathBuf) -> Option<Settings> {
        let data = fs::read_to_string(path).expect("Unable to read database file");
        Settings::from_str(&data)
    }

    fn from_str(s: &str) -> Option<Settings> {
        let setting: Settings = serde_json::from_str(s).expect("Problems");
        Some(setting)
    }
}

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
    None
}

#[derive(Debug, Clone)]
struct DatabaseFindError;

impl fmt::Display for DatabaseFindError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Failed to find datbase")
    }
}

use std::path::PathBuf;

fn find_database() -> Result<PathBuf, DatabaseFindError> {
    let top = find_top().ok_or(DatabaseFindError)?;

    let file_path = top.join(DBNAME);
    if file_path.exists() {
        Ok(file_path)
    } else {
        Err(DatabaseFindError)
    }
}

/// Starts a Docker container and runs a command inside it.
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
    // Read the JSON database from disk.
    let db_path = find_database().expect("Can not find database");
    let settings = Settings::from_path(db_path).expect("database reading problem");

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
        let command = container.command.as_str();

        let mut values = HashMap::new();

        // You need to keep the _a around, as to_string_lossy returns a reference to the pathbuf
        // You can either do to_string_lossy().into_owned() or you can just shadow the variable
        // Fix this section.
        let top_str_a = find_top().unwrap();
        let top_str = top_str_a.to_string_lossy();
        let pwd_str_a = std::env::current_dir().unwrap();
        let pwd_str = pwd_str_a.to_string_lossy();
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
