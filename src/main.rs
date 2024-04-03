use leon::Template;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::path::Path;
use std::process::Command;

static DBNAME: &str = "cmdwrap.json";

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
    fn lookup_progam(self: &Settings, prog_name: &str) -> Option<&Cmd> {
        self.commands.iter().find(|entry| entry.name == prog_name)
    }
}

impl Cmd {
    fn hello() {
        println!("Hello");
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
    let top = find_top().ok_or(DatabaseFindError)?.to_owned(); // `to_owned()` is used to avoid moving the path.

    let file_path = top.join(DBNAME);
    if file_path.exists() {
        Ok(file_path)
    } else {
        Err(DatabaseFindError)
    }
}

// Reads the JSON database from disk and return Settings
fn read_database(path: &Path) -> Option<Settings> {
    let data = fs::read_to_string(path).expect("Unable to read database file");
    let settings: Settings = serde_json::from_str(&data).expect("Invalid json data");
    Some(settings)
}

// Starts a Docker container and runs a command inside it.
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
    let settings = read_database(&db_path).expect("database reading problem");

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
