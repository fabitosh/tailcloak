use std::path::PathBuf;
use std::process::Command;

const CANDIDATE_INSTALLATION_PATHS: [&str; 4] = [
    "/usr/local/bin/tailscale",    // Tailscale.app CLI shim / Intel Homebrew
    "/opt/homebrew/bin/tailscale", // Apple Silicon Homebrew
    "/Applications/Tailscale.app/Contents/MacOS/Tailscale",
    "/usr/bin/tailscale",
];

pub fn resolve() -> Option<PathBuf> {
    CANDIDATE_INSTALLATION_PATHS
        .iter()
        .map(PathBuf::from)
        .find(|p| p.is_file()) // follows symlinks (Homebrew links into the Cellar)
}

/// A single Tailscale CLI invocation. `args()` is the pure mapping to argv,
/// so tests can assert the exact commands a policy produces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action<'a> {
    /// Plain `up` with no flags keeps the existing prefs; `up` with flags
    /// would demand every non-default pref be restated.
    Up,
    Down,
    /// `tailscale set` patches only the exit-node pref and leaves the up/down
    /// state untouched (works while down). `None` clears it. A node name or
    /// `auto:any` needs the netmap, so run `Up` first when setting one.
    SetExitNode(Option<&'a str>),
}

impl Action<'_> {
    pub fn args(&self) -> Vec<String> {
        match self {
            Action::Up => vec!["up".into()],
            Action::Down => vec!["down".into()],
            // One argv element with `=`, so an empty value (clear) can't be dropped.
            Action::SetExitNode(node) => {
                vec!["set".into(), format!("--exit-node={}", node.unwrap_or(""))]
            }
        }
    }

    pub fn run(&self) -> Result<(), Box<dyn std::error::Error>> {
        let tailscale = resolve().ok_or("tailscale CLI not found")?;
        let args = self.args();
        let status = Command::new(&tailscale).args(&args).status()?;
        if !status.success() {
            return Err(format!("tailscale {} exited with {status}", args.join(" ")).into());
        }
        Ok(())
    }
}

impl std::fmt::Display for Action<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Action::Up => write!(f, "tailscale up"),
            Action::Down => write!(f, "tailscale down"),
            Action::SetExitNode(None) => write!(f, "exit node off"),
            Action::SetExitNode(Some(node)) => write!(f, "exit node {node}"),
        }
    }
}

pub fn down() -> Result<(), Box<dyn std::error::Error>> {
    Action::Down.run()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn args_for_connection_state() {
        assert_eq!(Action::Up.args(), ["up"]);
        assert_eq!(Action::Down.args(), ["down"]);
    }

    #[test]
    fn args_set_exit_node() {
        assert_eq!(
            Action::SetExitNode(Some("auto:any")).args(),
            ["set", "--exit-node=auto:any"]
        );
    }

    #[test]
    fn args_clear_exit_node_with_empty_value() {
        assert_eq!(Action::SetExitNode(None).args(), ["set", "--exit-node="]);
    }
}
