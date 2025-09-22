use anyhow::{Context, Result};
use std::process::{Command, Stdio};
use std::time::Duration;
use tracing::{info, warn};

use crate::k8s_bootstrap::K3sConfig;

#[cfg(test)]
use crate::k8s_bootstrap::K3sInstallConfig;

pub struct K3sInstaller {
    pub config: K3sConfig,
    pub dry_run: bool,
}

#[derive(Debug, Clone)]
pub enum K3sStatus {
    NotInstalled,
    Installed,
    Running,
    Failed,
}

impl K3sInstaller {
    pub fn new(config: K3sConfig, dry_run: bool) -> Self {
        Self { config, dry_run }
    }

    pub fn install_k3s(&self) -> Result<()> {
        info!("Installing k3s version {}", self.config.version);

        if self.dry_run {
            self.print_install_plan()?;
            return Ok(());
        }

        if self.is_k3s_installed()? {
            info!("k3s is already installed, checking status...");
            let status = self.get_k3s_status()?;
            match status {
                K3sStatus::Running => {
                    info!("k3s is already running, skipping installation");
                    return Ok(());
                }
                K3sStatus::Installed => {
                    info!("k3s is installed but not running, attempting to start...");
                    self.start_k3s()?;
                    return Ok(());
                }
                _ => {
                    warn!("k3s is in an unexpected state, proceeding with installation");
                }
            }
        }

        self.download_and_install_k3s()?;
        self.wait_for_k3s_ready(Duration::from_secs(300))?;

        info!("k3s installation completed successfully");
        Ok(())
    }

    pub fn is_k3s_ready(&self) -> Result<bool> {
        if self.dry_run {
            return Ok(true);
        }

        let output = Command::new("k3s")
            .args(["kubectl", "get", "nodes", "--no-headers"])
            .output()
            .context("Failed to check k3s node status")?;

        if !output.status.success() {
            return Ok(false);
        }

        let output_str = String::from_utf8_lossy(&output.stdout);
        let ready_nodes = output_str
            .lines()
            .filter(|line| line.contains("Ready"))
            .count();

        Ok(ready_nodes > 0)
    }

    #[allow(dead_code)]
    pub fn uninstall_k3s(&self, force: bool) -> Result<()> {
        info!("Uninstalling k3s cluster");

        if self.dry_run {
            info!("[DRY RUN] Would uninstall k3s with force={}", force);
            return Ok(());
        }

        if !self.is_k3s_installed()? {
            info!("k3s is not installed, nothing to uninstall");
            return Ok(());
        }

        let uninstall_script = "/usr/local/bin/k3s-uninstall.sh";
        if std::path::Path::new(uninstall_script).exists() {
            let mut cmd = Command::new("sudo");
            cmd.arg(uninstall_script);

            if force {
                cmd.env("K3S_FORCE_RESTART", "true");
            }

            let status = cmd.status().context("Failed to run k3s uninstall script")?;

            if !status.success() {
                anyhow::bail!("k3s uninstall script failed with exit code: {}", status);
            }

            info!("k3s uninstalled successfully");
        } else {
            warn!("k3s uninstall script not found, k3s may not have been installed properly");
        }

        Ok(())
    }

    fn is_k3s_installed(&self) -> Result<bool> {
        let k3s_exists = Command::new("which")
            .arg("k3s")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .context("Failed to check if k3s is installed")?
            .success();

        Ok(k3s_exists)
    }

    pub fn get_k3s_status(&self) -> Result<K3sStatus> {
        if !self.is_k3s_installed()? {
            return Ok(K3sStatus::NotInstalled);
        }

        let status = Command::new("systemctl")
            .args(["is-active", "k3s"])
            .output()
            .context("Failed to check k3s service status")?;

        let status_str = String::from_utf8_lossy(&status.stdout).trim().to_string();

        match status_str.as_str() {
            "active" => {
                if self.is_k3s_ready()? {
                    Ok(K3sStatus::Running)
                } else {
                    Ok(K3sStatus::Installed)
                }
            }
            "inactive" | "failed" => Ok(K3sStatus::Installed),
            _ => Ok(K3sStatus::Failed),
        }
    }

    fn print_install_plan(&self) -> Result<()> {
        println!("📋 k3s Installation Plan:");
        println!("  Version: {}", self.config.version);
        println!("  Channel: {}", self.config.install.channel);
        println!("  Data Directory: {}", self.config.data_dir);
        println!("  Node Name: {}", self.config.node_name);

        if !self.config.install.disable.is_empty() {
            println!(
                "  Disabled Components: {}",
                self.config.install.disable.join(", ")
            );
        }

        if !self.config.extra_args.is_empty() {
            println!("  Extra Arguments: {}", self.config.extra_args.join(" "));
        }

        println!();
        println!("🔧 Install Command that would be executed:");
        println!("  curl -sfL https://get.k3s.io | \\");
        self.build_install_env_vars()
            .iter()
            .for_each(|(key, value)| {
                println!("    {}='{}' \\", key, value);
            });
        println!("    sh -s - \\");
        self.build_install_args().iter().for_each(|arg| {
            println!("      {} \\", arg);
        });
        println!();

        Ok(())
    }

    fn download_and_install_k3s(&self) -> Result<()> {
        info!("Downloading and installing k3s...");

        let mut cmd = Command::new("curl");
        cmd.args(["-sfL", "https://get.k3s.io"]);

        let mut install_cmd = Command::new("sh");
        install_cmd.args(["-s", "-"]);
        install_cmd.args(self.build_install_args());

        for (key, value) in self.build_install_env_vars() {
            install_cmd.env(key, value);
        }

        let curl_process = cmd
            .stdout(Stdio::piped())
            .spawn()
            .context("Failed to start curl process")?;

        install_cmd.stdin(curl_process.stdout.context("Failed to get curl stdout")?);

        let status = install_cmd
            .status()
            .context("Failed to execute k3s install script")?;

        if !status.success() {
            anyhow::bail!("k3s installation failed with exit code: {}", status);
        }

        Ok(())
    }

    fn build_install_env_vars(&self) -> Vec<(String, String)> {
        let mut env_vars = vec![];

        if !self.config.version.is_empty() && self.config.version != "latest" {
            env_vars.push((
                "INSTALL_K3S_VERSION".to_string(),
                self.config.version.clone(),
            ));
        }

        if self.config.install.channel != "stable" {
            env_vars.push((
                "INSTALL_K3S_CHANNEL".to_string(),
                self.config.install.channel.clone(),
            ));
        }

        env_vars
    }

    fn build_install_args(&self) -> Vec<String> {
        let mut args = vec!["server".to_string()];

        args.push("--data-dir".to_string());
        args.push(self.config.data_dir.clone());

        args.push("--node-name".to_string());
        args.push(self.config.node_name.clone());

        for component in &self.config.install.disable {
            args.push("--disable".to_string());
            args.push(component.clone());
        }

        args.extend(self.config.extra_args.clone());

        args
    }

    fn start_k3s(&self) -> Result<()> {
        info!("Starting k3s service...");

        let status = Command::new("sudo")
            .args(["systemctl", "start", "k3s"])
            .status()
            .context("Failed to start k3s service")?;

        if !status.success() {
            anyhow::bail!("Failed to start k3s service");
        }

        self.wait_for_k3s_ready(Duration::from_secs(120))?;
        Ok(())
    }

    fn wait_for_k3s_ready(&self, timeout: Duration) -> Result<()> {
        info!("Waiting for k3s cluster to be ready...");

        let start_time = std::time::Instant::now();
        let mut last_error = None;

        while start_time.elapsed() < timeout {
            match self.is_k3s_ready() {
                Ok(true) => {
                    info!("k3s cluster is ready!");
                    return Ok(());
                }
                Ok(false) => {
                    std::thread::sleep(Duration::from_secs(5));
                    continue;
                }
                Err(e) => {
                    last_error = Some(e);
                    std::thread::sleep(Duration::from_secs(5));
                    continue;
                }
            }
        }

        if let Some(err) = last_error {
            anyhow::bail!("k3s cluster failed to become ready within timeout: {}", err);
        } else {
            anyhow::bail!("k3s cluster failed to become ready within timeout");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_k3s_config() -> K3sConfig {
        K3sConfig {
            version: "v1.28.2+k3s1".to_string(),
            install: K3sInstallConfig {
                channel: "stable".to_string(),
                disable: vec!["traefik".to_string()],
            },
            data_dir: "/var/lib/rancher/k3s".to_string(),
            node_name: "test-node".to_string(),
            extra_args: vec!["--write-kubeconfig-mode=644".to_string()],
        }
    }

    #[test]
    fn given_k3s_installer_when_build_install_args_then_returns_correct_args() {
        let config = create_test_k3s_config();
        let installer = K3sInstaller::new(config, true);

        let args = installer.build_install_args();

        assert_eq!(args[0], "server");
        assert!(args.contains(&"--data-dir".to_string()));
        assert!(args.contains(&"/var/lib/rancher/k3s".to_string()));
        assert!(args.contains(&"--node-name".to_string()));
        assert!(args.contains(&"test-node".to_string()));
        assert!(args.contains(&"--disable".to_string()));
        assert!(args.contains(&"traefik".to_string()));
        assert!(args.contains(&"--write-kubeconfig-mode=644".to_string()));
    }

    #[test]
    fn given_k3s_installer_when_build_install_env_vars_then_returns_correct_vars() {
        let config = create_test_k3s_config();
        let installer = K3sInstaller::new(config, true);

        let env_vars = installer.build_install_env_vars();

        let version_var = env_vars.iter().find(|(k, _)| k == "INSTALL_K3S_VERSION");
        assert!(version_var.is_some());
        assert_eq!(version_var.unwrap().1, "v1.28.2+k3s1");
    }

    #[test]
    fn given_default_channel_when_build_install_env_vars_then_no_channel_var() {
        let mut config = create_test_k3s_config();
        config.install.channel = "stable".to_string();
        let installer = K3sInstaller::new(config, true);

        let env_vars = installer.build_install_env_vars();

        let channel_var = env_vars.iter().find(|(k, _)| k == "INSTALL_K3S_CHANNEL");
        assert!(channel_var.is_none());
    }

    #[test]
    fn given_custom_channel_when_build_install_env_vars_then_includes_channel_var() {
        let mut config = create_test_k3s_config();
        config.install.channel = "latest".to_string();
        let installer = K3sInstaller::new(config, true);

        let env_vars = installer.build_install_env_vars();

        let channel_var = env_vars.iter().find(|(k, _)| k == "INSTALL_K3S_CHANNEL");
        assert!(channel_var.is_some());
        assert_eq!(channel_var.unwrap().1, "latest");
    }

    #[test]
    fn given_dry_run_installer_when_install_k3s_then_shows_plan_without_executing() {
        let config = create_test_k3s_config();
        let installer = K3sInstaller::new(config, true);

        let result = installer.install_k3s();
        assert!(result.is_ok());
    }

    #[test]
    fn given_dry_run_installer_when_is_k3s_ready_then_returns_true() {
        let config = create_test_k3s_config();
        let installer = K3sInstaller::new(config, true);

        let result = installer.is_k3s_ready().unwrap();
        assert!(result);
    }

    #[test]
    fn given_dry_run_installer_when_uninstall_k3s_then_shows_plan_without_executing() {
        let config = create_test_k3s_config();
        let installer = K3sInstaller::new(config, true);

        let result = installer.uninstall_k3s(false);
        assert!(result.is_ok());
    }
}
