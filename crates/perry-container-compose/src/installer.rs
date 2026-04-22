use crate::backend::{detect_backend, BackendDriver};
use crate::error::{ComposeError, Result};
use console::Term;
use dialoguer::{theme::ColorfulTheme, Select};
use std::process::Command;

pub struct BackendInstaller {
    pub is_tty: bool,
}

impl BackendInstaller {
    pub fn new() -> Self {
        Self {
            is_tty: Term::stderr().is_term(),
        }
    }

    pub async fn run(&self) -> Result<BackendDriver> {
        if !self.is_tty {
            return Err(ComposeError::NoBackendFound { probed: vec![] });
        }

        if let Ok(_) = std::env::var("PERRY_NO_INSTALL_PROMPT") {
            return Err(ComposeError::NoBackendFound { probed: vec![] });
        }

        println!("Perry needs a container runtime to continue. No container runtime was found on this system.");

        let backends = self.get_platforms_backends();
        let items: Vec<String> = backends.iter().map(|(name, desc, _, _)| format!("{}: {}", name, desc)).collect();

        let selection = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("Select a backend to install")
            .items(&items)
            .default(0)
            .interact_opt()
            .map_err(|e| ComposeError::ValidationError { message: e.to_string() })?;

        if let Some(index) = selection {
            let (name, _desc, cmd, docs) = backends[index];
            println!("\nTo install {}, run:\n  {}\n\nDocs: {}\n", name, cmd, docs);

            let confirm = dialoguer::Confirm::new()
                .with_prompt(format!("Run '{}' now?", cmd))
                .interact()
                .map_err(|e| ComposeError::ValidationError { message: e.to_string() })?;

            if confirm {
                println!("Running installation command...");
                let status = if cfg!(target_os = "windows") {
                    Command::new("powershell").args(&["-Command", cmd]).status()?
                } else {
                    Command::new("sh").args(&["-c", cmd]).status()?
                };

                if status.success() {
                    println!("Installation completed. Re-probing...");
                    return crate::backend::probe_candidate_driver(name).await.map_err(|_e| ComposeError::NoBackendFound { probed: vec![] });
                } else {
                    return Err(ComposeError::BackendError { code: status.code().unwrap_or(1), message: "Installation failed".into() });
                }
            }
        }

        Err(ComposeError::NoBackendFound { probed: vec![] })
    }

    fn get_platforms_backends(&self) -> Vec<(&'static str, &'static str, &'static str, &'static str)> {
        if cfg!(target_os = "macos") {
            vec![
                ("apple/container", "Apple's native container runtime", "brew install container", "https://github.com/apple/container"),
                ("orbstack", "Fast macOS VM with Docker-compatible API", "brew install --cask orbstack", "https://orbstack.dev"),
                ("colima", "Lightweight macOS container runtime", "brew install colima", "https://github.com/abiosoft/colima"),
                ("podman", "Daemonless, rootless OCI runtime", "brew install podman && podman machine init && podman machine start", "https://podman.io"),
                ("docker", "Docker Desktop for Mac", "brew install --cask docker", "https://docs.docker.com/desktop/mac"),
            ]
        } else if cfg!(target_os = "linux") {
            vec![
                ("podman", "Daemonless, rootless OCI runtime", "sudo apt-get install -y podman", "https://podman.io/getting-started/installation"),
                ("docker", "Docker Engine", "curl -fsSL https://get.docker.com | sh", "https://docs.docker.com/engine/install"),
            ]
        } else {
            vec![
                ("podman", "Daemonless, rootless OCI runtime", "winget install RedHat.Podman", "https://podman.io/getting-started/installation"),
                ("docker", "Docker Desktop for Windows", "winget install Docker.DockerDesktop", "https://docs.docker.com/desktop/windows"),
            ]
        }
    }
}
