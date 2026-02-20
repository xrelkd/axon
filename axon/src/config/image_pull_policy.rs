//! Defines the `ImagePullPolicy` enum and its associated parsing logic.

use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};
use snafu::Snafu;

/// Represents the policy for pulling container images.
///
/// This enum defines strategies for how container images should be pulled from
/// a registry.
///
/// # Examples
///
/// Basic usage:
///
/// ```rust
/// use axon::config::ImagePullPolicy;
///
/// let policy_if_not_present = ImagePullPolicy::IfNotPresent;
/// let policy_always = ImagePullPolicy::Always;
/// let policy_never = ImagePullPolicy::Never;
///
/// assert_eq!(policy_if_not_present.to_string(), "IfNotPresent");
/// assert_eq!(policy_always.to_string(), "Always");
/// assert_eq!(policy_never.to_string(), "Never");
/// ```
#[derive(Clone, Debug, Default, Deserialize, Eq, Serialize, PartialEq)]
pub enum ImagePullPolicy {
    /// Pulls the image only if it is not already present locally.
    #[default]
    IfNotPresent,
    /// Always pulls the image, even if it is already present locally.
    Always,
    /// Never pulls the image; uses a local image only if it exists.
    Never,
}

impl fmt::Display for ImagePullPolicy {
    /// Formats the `ImagePullPolicy` into a human-readable string
    /// representation.
    ///
    /// The string representation matches the variant name.
    ///
    /// # Arguments
    ///
    /// * `f` - The formatter to write into.
    ///
    /// # Returns
    ///
    /// A `fmt::Result` indicating success or failure of the formatting
    /// operation.
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

    /// Parses a string into an `ImagePullPolicy`.
    ///
    /// This implementation is case-insensitive for the input string.
    /// Valid string values are `IfNotPresent`, `Always`, and `Never`.
    ///
    /// # Arguments
    ///
    /// * `value` - The string slice to parse.
    ///
    /// # Returns
    ///
    /// A `Result` which is `Ok(ImagePullPolicy)` if the string is a valid
    /// policy, or `Err(ParseImagePullPolicyError::Invalid)` if the string
    /// does not match any policy.
    ///
    /// # Errors
    ///
    /// Returns `ParseImagePullPolicyError::Invalid` if `value` does not
    /// correspond to a known `ImagePullPolicy` variant (e.g., "unknown").
    ///
    /// # Examples
    ///
    /// ```rust
    /// use std::str::FromStr;
    /// use axon::config::ImagePullPolicy;
    /// use axon::config::ParseImagePullPolicyError;
    ///
    /// assert_eq!(ImagePullPolicy::from_str("IfNotPresent").unwrap(), ImagePullPolicy::IfNotPresent);
    /// assert_eq!(ImagePullPolicy::from_str("ifnotpresent").unwrap(), ImagePullPolicy::IfNotPresent);
    /// assert_eq!(ImagePullPolicy::from_str("Always").unwrap(), ImagePullPolicy::Always);
    /// assert_eq!(ImagePullPolicy::from_str("never").unwrap(), ImagePullPolicy::Never);
    ///
    /// let err = ImagePullPolicy::from_str("InvalidPolicy").unwrap_err();
    /// assert!(matches!(err, ParseImagePullPolicyError::Invalid { value: _ }));
    /// ```
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.to_lowercase().as_str() {
            "ifnotpresent" => Ok(Self::IfNotPresent),
            "always" => Ok(Self::Always),
            "never" => Ok(Self::Never),
            _ => Err(ParseImagePullPolicyError::Invalid { value: value.to_string() }),
        }
    }
}

/// Represents an error that occurs during the parsing of an `ImagePullPolicy`
/// string.
#[derive(Debug, Snafu)]
pub enum ParseImagePullPolicyError {
    /// Indicates that the provided string value is not a valid
    /// `ImagePullPolicy`.
    #[snafu(display("'{value}' is not a valid ImagePullPolicy"))]
    Invalid { value: String },
}
