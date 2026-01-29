use std::cmp::Ordering;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl Version {
    pub fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }

    /// Parse version from string like "0.4.1" or "v0.4.1"
    pub fn parse(s: &str) -> Result<Self, String> {
        let s = s.trim().trim_start_matches('v');
        let parts: Vec<&str> = s.split('.').collect();

        if parts.len() != 3 {
            return Err(format!("Invalid version format: {}", s));
        }

        let major = parts[0]
            .parse()
            .map_err(|_| format!("Invalid major version: {}", parts[0]))?;
        let minor = parts[1]
            .parse()
            .map_err(|_| format!("Invalid minor version: {}", parts[1]))?;
        let patch = parts[2]
            .parse()
            .map_err(|_| format!("Invalid patch version: {}", parts[2]))?;

        Ok(Self::new(major, minor, patch))
    }

    /// Get the current application version from Cargo.toml
    pub fn current() -> Self {
        Self::parse(env!("CARGO_PKG_VERSION")).expect("Failed to parse current version")
    }

    /// Check if this version is newer than another
    pub fn is_newer_than(&self, other: &Version) -> bool {
        self > other
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.major.cmp(&other.major) {
            Ordering::Equal => match self.minor.cmp(&other.minor) {
                Ordering::Equal => self.patch.cmp(&other.patch),
                other => other,
            },
            other => other,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_parse() {
        assert_eq!(Version::parse("1.2.3").unwrap(), Version::new(1, 2, 3));
        assert_eq!(Version::parse("v1.2.3").unwrap(), Version::new(1, 2, 3));
        assert_eq!(Version::parse("0.4.1").unwrap(), Version::new(0, 4, 1));
    }

    #[test]
    fn test_version_parse_invalid_format() {
        assert!(Version::parse("1.2").is_err());
        assert!(Version::parse("1").is_err());
        assert!(Version::parse("").is_err());
        assert!(Version::parse("1.2.3.4").is_err());
    }

    #[test]
    fn test_version_parse_invalid_number() {
        assert!(Version::parse("a.b.c").is_err());
        assert!(Version::parse("1.x.3").is_err());
        assert!(Version::parse("1.2.three").is_err());
    }

    #[test]
    fn test_version_comparison() {
        let v1 = Version::new(1, 0, 0);
        let v2 = Version::new(1, 0, 1);
        let v3 = Version::new(1, 1, 0);
        let v4 = Version::new(2, 0, 0);

        assert!(v2.is_newer_than(&v1));
        assert!(v3.is_newer_than(&v2));
        assert!(v4.is_newer_than(&v3));
        assert!(!v1.is_newer_than(&v2));
    }

    #[test]
    fn test_version_equality() {
        let v1 = Version::new(1, 2, 3);
        let v2 = Version::new(1, 2, 3);
        assert_eq!(v1, v2);
    }

    #[test]
    fn test_version_is_newer_than_same() {
        let v1 = Version::new(1, 2, 3);
        let v2 = Version::new(1, 2, 3);
        assert!(!v1.is_newer_than(&v2));
    }

    #[test]
    fn test_version_current() {
        // Should successfully parse the current version from Cargo.toml
        let current = Version::current();
        let parsed =
            Version::parse(env!("CARGO_PKG_VERSION")).expect("Failed to parse package version");
        assert_eq!(current, parsed);
    }

    #[test]
    fn test_version_ord() {
        // Test full ordering: major > minor > patch
        let versions = vec![
            Version::new(0, 0, 1),
            Version::new(0, 0, 2),
            Version::new(0, 1, 0),
            Version::new(0, 1, 1),
            Version::new(1, 0, 0),
            Version::new(1, 0, 1),
            Version::new(1, 1, 0),
            Version::new(2, 0, 0),
        ];

        for i in 0..versions.len() - 1 {
            assert!(
                versions[i] < versions[i + 1],
                "{} should be < {}",
                versions[i],
                versions[i + 1]
            );
        }
    }

    #[test]
    fn test_version_display() {
        let v = Version::new(1, 2, 3);
        assert_eq!(format!("{}", v), "1.2.3");
    }
}
