extern crate env_logger;
extern crate log;

use std::{env, io};
use std::fs::File;
use std::io::Write;
use std::process::Command;

use log::{error, info, warn};
use regex::Regex;
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};

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

/// Ensures or installs Github CLI on Mac and Windows using brew and chocolatey, as well as a manual installer if needed
fn ensure_gh_installed() -> Result<(), String> {
    match Command::new("gh").arg("--version").output() {
        Ok(_) => {
            info!("GitHub CLI is already installed.");
            Ok(())
        }
        Err(_) => {
            // Inform the user why GitHub CLI is needed
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
                        // Use brew on macOS
                        Command::new("brew").args(&["install", "gh"]).status()
                            .map_err(|e| format!("Failed to install GitHub CLI: {}", e))?;
                        Ok(())
                    }
                    "windows" => {
                        // Check for Chocolatey
                        if Command::new("choco").arg("--version").output().is_ok() {
                            info!("Found Chocolatey! Installing GitHub CLI...");
                            let status = Command::new("powershell")
                                .args(&[
                                    "-Command",
                                    "Start-Process",
                                    "choco",
                                    "-ArgumentList",
                                    "'install', 'gh', '-y'",
                                    "-Verb",
                                    "RunAs"
                                ])
                                .status()
                                .map_err(|e| format!("Failed to launch Chocolatey with elevated privileges: {}", e))?;

                            return if status.success() {
                                Ok(())
                            } else {
                                Err("Failed to install GitHub CLI using Chocolatey with elevated privileges.".to_string())
                            }
                        }

                        // If Chocolatey is not found, download the executable
                        // Determine the path to the Downloads folder
                        let downloads_folder = dirs::download_dir().ok_or("Unable to determine the Downloads folder path.".to_string())?;
                        let installer_path = downloads_folder.join("gh_installer.msi");

                        Command::new("powershell")
                            .args(&["-Command", "Invoke-WebRequest", "-Uri", "https://github.com/cli/cli/releases/download/v2.0.0/gh_2.0.0_windows_amd64.msi", "-OutFile", &installer_path.to_string_lossy()])
                            .status()
                            .map_err(|e| format!("Failed to download GitHub CLI installer: {}", e))?;

                        info!("Installer downloaded to '{}'. Please install it manually.", installer_path.to_string_lossy());
                        Err("Please install the GitHub CLI using the downloaded 'gh_installer.msi' and rerun the program.".to_string())
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
    // Check if GITHUB_TOKEN is set
    if env::var("GITHUB_TOKEN").is_ok() {
        info!("Clearing the GITHUB_TOKEN environment variable...");
        env::remove_var("GITHUB_TOKEN");
    }

    match run_command("gh", &["auth", "status"]) {
        Ok(_) => Ok(()),
        Err(_) => {
            info!("You are not logged in to GitHub via 'gh' CLI.");
            info!("Please follow the on-screen instructions to authenticate.");
            run_command("gh", &["auth", "login"]).map(|_| ())
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
        print!("Enter the name for the new GitHub repository: ");  // Changed info! to print!
        io::stdout().flush().unwrap();  // Flush stdout to display the prompt before input
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
        print!("Should the repository be public or private? (public/private): ");  // Changed info! to print!
        io::stdout().flush().unwrap();  // Flush stdout to display the prompt before input
        io::stdin().read_line(&mut repo_visibility).expect("Failed to read line");
        repo_visibility = repo_visibility.trim().to_string();

        if ["public", "private"].contains(&repo_visibility.as_str()) {
            break;
        } else {
            warn!("Invalid option. Please choose 'public' or 'private'.");
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
}

/// Create LICENSE and README.md files.
fn create_license_and_readme(repo_name: &str) -> Result<(), io::Error> {
    let mut file = File::create("README.md")?;
    writeln!(file, "# {}", repo_name)?;

    let mut file = File::create("LICENSE")?;
    file.write_all(b"GNU GENERAL PUBLIC LICENSE\nVersion 3, 29 June 2007\n")?;
    Ok(())
}

fn write_info_in_green(text: &str) -> io::Result<()> {
    let mut stdout = StandardStream::stdout(ColorChoice::Always);
    stdout.set_color(ColorSpec::new().set_fg(Some(Color::Green)))?;
    writeln!(&mut stdout, "{}", text)
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format(|buf, record| {
            match record.level() {
                log::Level::Info => write_info_in_green(&format!("{}", record.args())),
                _ => writeln!(buf, "{}", record.args())
            }
        })
        .init();

    // Ensure GitHub CLI is installed before proceeding
    if let Err(err) = ensure_gh_installed() {
        error!("Error: {}", err);
        std::process::exit(1);
    }

    // Check if 'gh' is available after downloading the installer
    if Command::new("gh").arg("--version").output().is_err() {
        info!("Please install the GitHub CLI using the downloaded 'gh_installer.msi' and rerun the program.");
        std::process::exit(0); // Exit without error
    }

    if let Err(err) = check_gh_authenticated() {
        error!("Error: {}", err);
        std::process::exit(1);
    }

    let (repo_name, repo_visibility) = get_repo_details();

    match create_github_repo(&repo_name, &repo_visibility) {
        Ok(github_url) => {
            if let Err(err) = handle_git_remote(&github_url) {
                error!("Error: {}", err);
                std::process::exit(1);
            }

            if let Err(err) = create_license_and_readme(&repo_name) {
                error!("Failed to create LICENSE and README.md files: {}", err);
                std::process::exit(1);
            }

            info!("GitHub repository initialized and linked. You can now manually add, commit, and push files.");
        }
        Err(err) => {
            error!("Error: {}", err);
            std::process::exit(1);
        }
    }
}