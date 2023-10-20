extern crate env_logger;
extern crate log;

use std::{env, io};
use std::io::Write;
use std::process::Command;

use log::{error, info, warn};
use regex::Regex;

/// Print an introductory message describing the program.
fn print_intro() {
    println!("\n🦆 Welcome to Quack! 🦆");
    println!("\nMaking your GitHub life easier by:");
    println!("  - Ensuring GitHub CLI is installed");
    println!("  - Authenticating you with GitHub");
    println!("  - Creating a new GitHub repository");
    println!("  - Linking the new repo to your local repo");
    println!("\nLet's get started!");
}


/// Run shell command and capture the output.
fn run_command(command: &str, args: &[&str]) -> Result<String, String> {
    let output = Command::new(command)
        .args(args)
        .output()
        .expect("Failed to execute command");

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

/// Ensures or installs Github CLI.
fn ensure_gh_installed() -> Result<(), String> {
    match Command::new("gh").arg("--version").output() {
        Ok(_) => {
            info!("✅  GitHub CLI is already installed.");
            Ok(())
        }
        Err(_) => {
            info!("The GitHub CLI is required for authentication, repository creation, and other GitHub operations.");
            print!("Do you want to install it? (y/n): ");
            io::stdout().flush().unwrap();
            let mut input = String::new();
            io::stdin().read_line(&mut input).expect("Failed to read line");
            let input = input.trim().to_lowercase();

            if input == "y" || input == "yes" {
                let os = env::consts::OS;
                match os {
                    "macos" => {
                        Command::new("brew").args(&["install", "gh"]).status()
                            .map_err(|e| format!("Failed to install GitHub CLI: {}", e))?;
                        Ok(())
                    }
                    "windows" => {
                        let output = Command::new("winget")
                            .args(&["install", "--id", "GitHub.cli"])
                            .output()
                            .map_err(|e| format!("Failed to launch winget: {}", e))?;

                        let output_str = String::from_utf8_lossy(&output.stderr);
                        if output.status.success() || output_str.contains("No newer package versions are available") {
                            Ok(())
                        } else {
                            Err("Failed to install GitHub CLI using winget.".to_string())
                        }
                    }
                    _ => Err("Unsupported operating system.".to_string())
                }
            } else {
                Err("User opted not to install the GitHub CLI.".to_string())
            }
        }
    }
}


/// Check if the user is authenticated with GitHub.
fn check_gh_authenticated() -> Result<(), String> {
    match run_command("gh", &["auth", "status"]) {
        Ok(_) => {
            info!("✅  You are already authenticated with GitHub.");
            Ok(())
        }
        Err(_) => {
            info!("You are not logged in to GitHub via 'gh' CLI.");
            info!("Attempting automated authentication with predefined choices.");

            let status = Command::new("gh")
                .args(&[
                    "auth",
                    "login",
                    "-p",
                    "https",
                    "-w",  // To open a web browser for authentication
                ])
                .status()
                .expect("Failed to execute 'gh auth login'");

            if status.success() {
                // Set the git protocol to HTTPS
                Command::new("gh")
                    .args(&["config", "set", "-h", "github.com", "git_protocol", "https"])
                    .status()
                    .expect("Failed to set git protocol to HTTPS");
                Ok(())
            } else {
                Err("Automated authentication failed.".to_string())
            }
        }
    }
}


/// Validate repository name against GitHub rules.
fn is_valid_repo_name(name: &str) -> bool {
    let re = Regex::new(r"^[a-zA-Z0-9_.-]+$").unwrap();
    re.is_match(name)
}

/// Get user input for repository name and visibility.
fn get_repo_details() -> (String, String) {
    let mut repo_name = String::new();
    let mut repo_visibility = String::new();

    loop {
        print!("\nNew repo name?: ");
        io::stdout().flush().unwrap();
        io::stdin().read_line(&mut repo_name).expect("Failed to read line");
        repo_name = repo_name.trim().to_string();

        if is_valid_repo_name(&repo_name) {
            break;
        } else {
            warn!("Invalid repository name. Only alphanumeric characters and '.', '-', '_' are allowed.");
            repo_name.clear();
        }
    }

    loop {
        print!("Make repo public? Y/n: ");
        io::stdout().flush().unwrap();
        io::stdin().read_line(&mut repo_visibility).expect("Failed to read line");
        repo_visibility = repo_visibility.trim().to_string();

        // Default to "public" if nothing or just "Y" is entered
        if repo_visibility.is_empty() || repo_visibility.to_lowercase() == "y" {
            repo_visibility = "public".to_string();
            break;
        }
        // Accept "n" for private repositories
        else if repo_visibility.to_lowercase() == "n" {
            repo_visibility = "private".to_string();
            break;
        } else {
            warn!("Invalid option. Type 'Y' for public or 'n' for private.");
            repo_visibility.clear();
        }
    }

    (repo_name, repo_visibility)
}


/// Create a new GitHub repository.
fn create_github_repo(repo_name: &str, repo_visibility: &str) -> Result<String, String> {
    match run_command("gh", &["repo", "create", repo_name, &format!("--{}", repo_visibility), "--confirm"]) {
        Ok(output) => {
            for line in output.lines() {
                if line.contains("git@") || line.contains("https://") {
                    return Ok(line.trim().to_string());
                }
            }
            Err("Could not capture GitHub URL.".to_string())
        }
        Err(err) => Err(format!("Could not create GitHub repository: {}", err))
    }
}

/// Initialize git and set remote URL.
fn handle_git_remote(new_github_url: &str) -> Result<String, String> {
    // Prompt user about setting git remotes
    print!("Link local repo with new repo? (Y/n): ");
    io::stdout().flush().unwrap();
    let mut input = String::new();
    io::stdin().read_line(&mut input).expect("Failed to read line");
    let input = input.trim().to_lowercase();

    // Default to "yes" if input is empty
    if input.is_empty() || input == "y" || input == "yes" {
        run_command("git", &["init"])?;

        match run_command("git", &["remote"]) {
            Ok(output) => {
                if output.contains("origin") {
                    run_command("git", &["remote", "set-url", "origin", new_github_url])
                } else {
                    run_command("git", &["remote", "add", "origin", new_github_url])
                }
            }
            Err(err) => Err(format!("Could not set git remote: {}", err))
        }
    } else {
        info!("Skipped setting git remotes.");
        Ok("Skipped".to_string())
    }
}

fn main() {
    // Print the introductory message
    print_intro();

    // Initialize the logger
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format(|buf, record| {
            writeln!(buf, "{}", record.args())
        })
        .init();

    // Ensure GitHub CLI is installed
    if let Err(err) = ensure_gh_installed() {
        error!("GitHub CLI Error: {}", err);
        std::process::exit(1);
    }

    // Authentication
    if let Err(err) = check_gh_authenticated() {
        error!("Authentication Error: {}", err);
        std::process::exit(1);
    }

    let (repo_name, repo_visibility) = get_repo_details();

    match create_github_repo(&repo_name, &repo_visibility) {
        Ok(github_url) => {
            let git_remote_result = handle_git_remote(&github_url);
            match git_remote_result {
                Ok(msg) => {
                    if msg == "Skipped" {
                        info!("GitHub repository created.");
                    } else {
                        info!("GitHub repository created and linked. You can now manually add, commit, and push files.");
                    }
                }
                Err(err) => {
                    error!("Error: {}", err);
                    std::process::exit(1);
                }
            }
        }
        Err(err) => {
            error!("Error: {}", err);
            std::process::exit(1);
        }
    }
}
