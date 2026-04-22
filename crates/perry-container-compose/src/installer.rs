use crate::error::{ComposeError, Result};
use crate::backend::{detect_backend, BackendDriver};

#[cfg(feature = "installer")]
use dialoguer::{theme::ColorfulTheme, Select};
#[cfg(feature = "installer")]
use console::style;

pub struct BackendInstaller;

#[derive(Clone)]
struct BackendOption {
    name: &'static str,
    description: &'static str,
    install_command: &'static str,
    docs_url: &'static str,
}

impl BackendInstaller {
    pub async fn run() -> Result<BackendDriver> {
        #[cfg(not(feature = "installer"))]
        {
            return Err(ComposeError::NoBackendFound { probed: vec![] });
        }

        #[cfg(feature = "installer")]
        {
            if !console::Term::stderr().is_term() || std::env::var("PERRY_NO_INSTALL_PROMPT").is_ok() {
                return Err(ComposeError::NoBackendFound { probed: vec![] });
            }

            println!("{}", style("Perry needs a container runtime to continue.").bold());
            println!("No container runtime was found on this system.\n");

            let options = Self::get_options();
            let selection = Select::with_theme(&ColorfulTheme::default())
                .with_prompt("Select a backend to install")
                .items(&options.iter().map(|o| format!("{} - {}", o.name, o.description)).collect::<Vec<_>>())
                .default(0)
                .interact_opt()
                .map_err(|e| ComposeError::validation(e.to_string()))?;

            if let Some(index) = selection {
                let option = &options[index];
                println!("\nTo install {}, run:", style(option.name).cyan());
                println!("  {}\n", style(option.install_command).bold());
                println!("Documentation: {}\n", style(option.docs_url).underlined());

                println!("Would you like to run the install command now?");
                let confirm = dialoguer::Confirm::with_theme(&ColorfulTheme::default())
                    .with_prompt("Run installation")
                    .interact()
                    .map_err(|e| ComposeError::validation(e.to_string()))?;

                if confirm {
                    let mut child = tokio::process::Command::new("sh")
                        .arg("-c")
                        .arg(option.install_command)
                        .spawn()?;

                    let status = child.wait().await?;
                    if status.success() {
                        println!("{}", style("Installation successful!").green());
                        match detect_backend().await {
                            Ok(b) => Ok(b.driver),
                            Err(e) => Err(ComposeError::NoBackendFound { probed: e }),
                        }
                    } else {
                        Err(ComposeError::validation("Installation failed"))
                    }
                } else {
                    Err(ComposeError::NoBackendFound { probed: vec![] })
                }
            } else {
                Err(ComposeError::NoBackendFound { probed: vec![] })
            }
        }
    }

    fn get_options() -> Vec<BackendOption> {
        if cfg!(target_os = "macos") || cfg!(target_os = "ios") {
            vec![
                BackendOption {
                    name: "apple/container",
                    description: "Apple's native container runtime (recommended)",
                    install_command: "brew install container",
                    docs_url: "https://github.com/apple/container",
                },
                BackendOption {
                    name: "orbstack",
                    description: "Fast macOS VM with Docker-compatible API",
                    install_command: "brew install --cask orbstack",
                    docs_url: "https://orbstack.dev",
                },
                BackendOption {
                    name: "colima",
                    description: "Lightweight macOS container runtime",
                    install_command: "brew install colima",
                    docs_url: "https://github.com/abiosoft/colima",
                },
                BackendOption {
                    name: "podman",
                    description: "Daemonless, rootless OCI runtime",
                    install_command: "brew install podman && podman machine init && podman machine start",
                    docs_url: "https://podman.io",
                },
                BackendOption {
                    name: "docker",
                    description: "Docker Desktop for Mac",
                    install_command: "brew install --cask docker",
                    docs_url: "https://docs.docker.com/desktop/mac",
                },
            ]
        } else if cfg!(target_os = "linux") {
            vec![
                BackendOption {
                    name: "podman",
                    description: "Daemonless, rootless OCI runtime (recommended)",
                    install_command: "sudo apt-get install -y podman",
                    docs_url: "https://podman.io/getting-started/installation",
                },
                BackendOption {
                    name: "docker",
                    description: "Docker Engine",
                    install_command: "curl -fsSL https://get.docker.com | sh",
                    docs_url: "https://docs.docker.com/engine/install",
                },
            ]
        } else {
            vec![
                BackendOption {
                    name: "podman",
                    description: "Daemonless, rootless OCI runtime (recommended)",
                    install_command: "winget install RedHat.Podman",
                    docs_url: "https://podman.io/getting-started/installation",
                },
                BackendOption {
                    name: "docker",
                    description: "Docker Desktop for Windows",
                    install_command: "winget install Docker.DockerDesktop",
                    docs_url: "https://docs.docker.com/desktop/windows",
                },
            ]
        }
    }
}
