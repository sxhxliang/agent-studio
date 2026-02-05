use std::path::Path;

/// Supported package managers across platforms
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageManager {
    /// Windows: Chocolatey
    Chocolatey,
    /// Windows: Winget (Windows Package Manager)
    Winget,
    /// Windows: Scoop
    Scoop,
    /// macOS: Homebrew
    Homebrew,
    /// Linux: APT (Debian/Ubuntu)
    Apt,
    /// Linux: YUM (RedHat/CentOS)
    Yum,
    /// Linux: DNF (Fedora)
    Dnf,
    /// Linux: Pacman (Arch Linux)
    Pacman,
    /// Unknown or no package manager detected
    Unknown,
}

impl PackageManager {
    /// Get the install command for this package manager
    pub fn install_command(&self) -> &'static str {
        match self {
            PackageManager::Chocolatey => "choco install nodejs",
            PackageManager::Winget => "winget install OpenJS.NodeJS",
            PackageManager::Scoop => "scoop install nodejs",
            PackageManager::Homebrew => "brew install node",
            PackageManager::Apt => "sudo apt install nodejs npm",
            PackageManager::Yum => "sudo yum install nodejs npm",
            PackageManager::Dnf => "sudo dnf install nodejs npm",
            PackageManager::Pacman => "sudo pacman -S nodejs npm",
            PackageManager::Unknown => "",
        }
    }

    /// Get a user-friendly name for this package manager
    pub fn name(&self) -> &'static str {
        match self {
            PackageManager::Chocolatey => "Chocolatey",
            PackageManager::Winget => "Winget",
            PackageManager::Scoop => "Scoop",
            PackageManager::Homebrew => "Homebrew",
            PackageManager::Apt => "APT",
            PackageManager::Yum => "YUM",
            PackageManager::Dnf => "DNF",
            PackageManager::Pacman => "Pacman",
            PackageManager::Unknown => "Unknown",
        }
    }
}

/// Detect available package manager on the current system
pub async fn detect_package_manager() -> PackageManager {
    // Platform-specific detection
    #[cfg(target_os = "windows")]
    {
        // Check for Chocolatey first (most common for dev tools)
        if command_exists("choco").await {
            return PackageManager::Chocolatey;
        }
        // Check for Winget (built into Windows 11)
        if command_exists("winget").await {
            return PackageManager::Winget;
        }
        // Check for Scoop
        if command_exists("scoop").await {
            return PackageManager::Scoop;
        }
    }

    #[cfg(target_os = "macos")]
    {
        // Check for Homebrew (standard on macOS)
        if Path::new("/usr/local/bin/brew").exists()
            || Path::new("/opt/homebrew/bin/brew").exists()
            || command_exists("brew").await
        {
            return PackageManager::Homebrew;
        }
    }

    #[cfg(target_os = "linux")]
    {
        // Check for APT (Debian/Ubuntu)
        if Path::new("/usr/bin/apt").exists() || Path::new("/usr/bin/apt-get").exists() {
            return PackageManager::Apt;
        }
        // Check for DNF (Fedora)
        if Path::new("/usr/bin/dnf").exists() {
            return PackageManager::Dnf;
        }
        // Check for YUM (RedHat/CentOS)
        if Path::new("/usr/bin/yum").exists() {
            return PackageManager::Yum;
        }
        // Check for Pacman (Arch Linux)
        if Path::new("/usr/bin/pacman").exists() {
            return PackageManager::Pacman;
        }
    }

    PackageManager::Unknown
}

/// Generate a helpful installation hint for the user
pub async fn generate_install_hint() -> String {
    let pm = detect_package_manager().await;

    let mut hint = String::new();

    if pm != PackageManager::Unknown {
        hint.push_str(&format!(
            "Install using {} ({}

):\n   {}\n\n",
            pm.name(),
            get_platform_name(),
            pm.install_command()
        ));
    }

    // Always provide manual download option
    hint.push_str("Or download manually from:\n");
    hint.push_str("   https://nodejs.org/\n\n");

    // Platform-specific additional guidance
    #[cfg(target_os = "windows")]
    {
        if pm == PackageManager::Unknown {
            hint.push_str("You can also install Chocolatey first:\n");
            hint.push_str("   https://chocolatey.org/install\n\n");
        }
    }

    #[cfg(target_os = "macos")]
    {
        if pm == PackageManager::Unknown {
            hint.push_str("You can also install Homebrew first:\n");
            hint.push_str("   https://brew.sh/\n\n");
        }
    }

    #[cfg(target_os = "linux")]
    {
        hint.push_str("Or use nvm (Node Version Manager):\n");
        hint.push_str("   curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.39.0/install.sh | bash\n\n");
    }

    hint.push_str("After installation, you may need to:\n");
    hint.push_str("1. Restart this application\n");
    hint.push_str("2. Or configure the Node.js path in: Settings > General > Node.js Path");

    hint
}

/// Check if a command exists in PATH
async fn command_exists(command: &str) -> bool {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        let mut cmd = tokio::process::Command::new("where");
        cmd.arg(command);
        cmd.creation_flags(CREATE_NO_WINDOW);
        cmd.output()
            .await
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    #[cfg(not(target_os = "windows"))]
    {
        tokio::process::Command::new("which")
            .arg(command)
            .output()
            .await
            .map(|output| output.status.success())
            .unwrap_or(false)
    }
}

/// Get platform-specific name for display
fn get_platform_name() -> &'static str {
    #[cfg(target_os = "windows")]
    {
        "Windows"
    }
    #[cfg(target_os = "macos")]
    {
        "macOS"
    }
    #[cfg(target_os = "linux")]
    {
        "Linux"
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        "Unknown OS"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_package_manager_commands() {
        assert_eq!(
            PackageManager::Chocolatey.install_command(),
            "choco install nodejs"
        );
        assert_eq!(
            PackageManager::Homebrew.install_command(),
            "brew install node"
        );
        assert_eq!(
            PackageManager::Apt.install_command(),
            "sudo apt install nodejs npm"
        );
    }

    #[tokio::test]
    async fn test_detect_package_manager() {
        // This test will return different results based on the system
        let pm = detect_package_manager().await;
        assert!(pm != PackageManager::Unknown || cfg!(target_os = "windows"));
    }

    #[tokio::test]
    async fn test_generate_install_hint() {
        let hint = generate_install_hint().await;
        assert!(!hint.is_empty());
        assert!(hint.contains("nodejs.org"));
    }
}
