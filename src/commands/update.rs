#[cfg(target_os = "linux")]
use std::path::Path;
#[cfg(target_os = "linux")]
use std::process::Command;

use anyhow::Result;
use baudbound_storage::SqliteRunnerStore;
use serde::Serialize;

use crate::updates;

pub fn check(store: &SqliteRunnerStore, json: bool) -> Result<()> {
    let result = updates::check_now(store)?;
    let installation = current_installation();
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&CliUpdateResult {
                result: &result,
                installation_type: installation.name,
                update_method: installation.update_method,
            })?
        );
    } else if result.update_available {
        println!(
            "BaudBound {} is available. You are running {}.",
            result.latest_version, result.current_version
        );
        println!("Installation type: {}.", installation.name);
        println!("{}", installation.instructions);
    } else {
        println!("BaudBound {} is up to date.", result.current_version);
        println!("Installation type: {}.", installation.name);
    }
    Ok(())
}

#[derive(Serialize)]
struct CliUpdateResult<'a> {
    #[serde(flatten)]
    result: &'a updates::UpdateCheckResult,
    installation_type: &'static str,
    update_method: &'static str,
}

#[derive(Clone, Copy)]
struct Installation {
    name: &'static str,
    update_method: &'static str,
    instructions: &'static str,
}

const RELEASE_INSTRUCTIONS: &str =
    "Open https://github.com/BaudBound/baudbound/releases/latest for installation options.";
#[cfg(target_os = "linux")]
const NATIVE_LINUX_INSTRUCTIONS: &str = "Stop active runs and the background runner, fully quit BaudBound, then run:\ncurl -fsSL https://get.baudbound.app/linux | sh";

fn current_installation() -> Installation {
    #[cfg(target_os = "linux")]
    {
        if std::env::var_os("APPIMAGE").is_some_and(|value| !value.is_empty()) {
            return Installation {
                name: "Linux AppImage",
                update_method: "in-app updater or GitHub Release",
                instructions: RELEASE_INSTRUCTIONS,
            };
        }

        let executable = std::env::current_exe().ok();
        if executable.as_deref().is_some_and(is_system_executable) {
            if package_owns_executable("dpkg-query", &["--search", "/usr/bin/baudbound"]) {
                return native_linux_installation("Debian package", "APT");
            }
            if package_owns_executable("rpm", &["-qf", "/usr/bin/baudbound"]) {
                return native_linux_installation("RPM package", "DNF");
            }
        }

        Installation {
            name: "unpackaged Linux executable",
            update_method: "GitHub Release",
            instructions: RELEASE_INSTRUCTIONS,
        }
    }

    #[cfg(target_os = "windows")]
    {
        Installation {
            name: "Windows installer",
            update_method: "in-app updater or GitHub Release",
            instructions: RELEASE_INSTRUCTIONS,
        }
    }

    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        Installation {
            name: "unpackaged executable",
            update_method: "GitHub Release",
            instructions: RELEASE_INSTRUCTIONS,
        }
    }
}

#[cfg(target_os = "linux")]
fn native_linux_installation(name: &'static str, package_manager: &'static str) -> Installation {
    Installation {
        name,
        update_method: package_manager,
        instructions: NATIVE_LINUX_INSTRUCTIONS,
    }
}

#[cfg(target_os = "linux")]
fn is_system_executable(path: &Path) -> bool {
    path == Path::new("/usr/bin/baudbound")
}

#[cfg(target_os = "linux")]
fn package_owns_executable(command: &str, arguments: &[&str]) -> bool {
    Command::new(command)
        .args(arguments)
        .output()
        .is_ok_and(|output| output.status.success())
}
