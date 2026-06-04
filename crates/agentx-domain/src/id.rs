//! Strongly-typed identifiers.
//!
//! Each id is a transparent newtype around a `String`. Using distinct types
//! prevents a whole class of bugs from the legacy code, where everything was a
//! bare `String` and it was easy to pass a session id where an agent id was
//! expected. The compiler now rejects that.

use std::fmt;

use serde::{Deserialize, Serialize};

macro_rules! id_newtype {
    ($name:ident) => {
        #[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Self {
                Self(value.into())
            }
            pub fn as_str(&self) -> &str {
                &self.0
            }
            pub fn into_string(self) -> String {
                self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl From<String> for $name {
            fn from(value: String) -> Self {
                Self(value)
            }
        }

        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                Self(value.to_owned())
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                &self.0
            }
        }
    };
}

id_newtype!(AgentId);
id_newtype!(SessionId);
id_newtype!(WorkspaceId);
id_newtype!(TaskId);
id_newtype!(PermissionId);

macro_rules! generated_id {
    ($name:ident) => {
        impl $name {
            /// Generate a fresh random id (UUID v4).
            pub fn generate() -> Self {
                Self(uuid::Uuid::new_v4().to_string())
            }
        }
    };
}

// Agent ids come from configuration (the agent's name), so they are never
// generated. The rest are minted by the application.
generated_id!(SessionId);
generated_id!(WorkspaceId);
generated_id!(TaskId);
generated_id!(PermissionId);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_transparently_as_a_string() {
        let id = SessionId::from("abc");
        assert_eq!(id.as_str(), "abc");
        assert_eq!(id.to_string(), "abc");
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, "\"abc\"");
        let parsed: SessionId = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, id);
    }

    #[test]
    fn generated_ids_are_unique() {
        assert_ne!(SessionId::generate(), SessionId::generate());
    }
}
