use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};
use snafu::Snafu;

#[derive(Clone, Debug, Default, Deserialize, Eq, Serialize, PartialEq)]
pub enum ImagePullPolicy {
    #[default]
    IfNotPresent,
    Always,
    Never,
}

impl fmt::Display for ImagePullPolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let val = match self {
            Self::IfNotPresent => "IfNotPresent",
            Self::Always => "Always",
            Self::Never => "Never",
        };
        f.write_str(val)
    }
}

impl FromStr for ImagePullPolicy {
    type Err = ParseImagePullPolicyError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.to_lowercase().as_str() {
            "ifnotpresent" => Ok(Self::IfNotPresent),
            "always" => Ok(Self::Always),
            "never" => Ok(Self::Never),
            _ => Err(ParseImagePullPolicyError::Invalid { value: value.to_string() }),
        }
    }
}

#[derive(Debug, Snafu)]
pub enum ParseImagePullPolicyError {
    #[snafu(display("'{value}' is not a valid ImagePullPolicy"))]
    Invalid { value: String },
}
