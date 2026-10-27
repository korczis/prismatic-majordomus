//! The facts of the platform hosting this process, read by name and from nothing else.
//!
//! A hosted Machine is handed its configuration through the environment, and the same
//! environment carries what must never leave it: deploy tokens, database URLs, signing
//! secrets. So nothing here reflects the environment. [`ALLOWLIST`] names every variable
//! this module may read, each one a fact of where the process runs that is safe to publish,
//! and [`Platform::read`] asks for exactly those names, one by one. The answer has one field
//! per name and no room for anything else: a variable the list does not name is never read,
//! so it cannot reach an answer, whatever it is called.
//!
//! The list and the answer's shape are one declaration (the `platform_facts!` table below),
//! so a name cannot be read without being a field, nor be a field without being on the list.
//! The network addresses a platform injects (`FLY_PUBLIC_IP`, `FLY_PRIVATE_IP`) are left
//! off deliberately: they map the platform's network rather than say which process this is.
//!
//! Absence is a fact too. Outside a hosting platform every field is `null` rather than an
//! error, and a variable set to nothing but whitespace is the same absence.
//!
//! ```
//! use majordomus_cli::platform::{Platform, ALLOWLIST};
//!
//! // on a Machine: the allowlisted names answer, and nothing else is ever asked for
//! let mut asked = Vec::new();
//! let platform = Platform::read(|name| {
//!     asked.push(name.to_string());
//!     match name {
//!         "FLY_REGION" => Some("ams".to_string()),
//!         _ => Some("anything".to_string()),
//!     }
//! });
//! assert_eq!(platform.fly_region.as_deref(), Some("ams"));
//! assert_eq!(asked, ALLOWLIST);
//!
//! // anywhere else: absent, not an error
//! let bare = Platform::read(|_| None);
//! assert_eq!(bare, Platform::default());
//! assert!(bare.present().is_empty());
//! ```

use schemars::JsonSchema;
use serde::Serialize;

/// One table declares the allowlist, the answer's fields and the read, so that the three
/// cannot disagree. Each entry is a field, the variable it is read from, and its doc
/// comment, which the generated schema carries as the field's description.
macro_rules! platform_facts {
    ($( $(#[doc = $doc:literal])* $field:ident = $var:literal ),+ $(,)?) => {
        /// The facts of the hosting platform, each read from the one variable its field
        /// names and `null` when that variable is unset or empty. Nothing outside these
        /// fields is read from the environment.
        #[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, JsonSchema)]
        pub struct Platform {
            $( $(#[doc = $doc])* pub $field: Option<String>, )+
        }

        /// Every environment variable this module may read, in the order it reads them.
        pub const ALLOWLIST: &[&str] = &[$( $var ),+];

        impl Platform {
            /// Read every allowlisted variable through `lookup`, and nothing else.
            ///
            /// `lookup` is handed exactly the names of [`ALLOWLIST`], once each, in order;
            /// [`Platform::from_env`] hands it the process environment.
            pub fn read(mut lookup: impl FnMut(&str) -> Option<String>) -> Self {
                Self { $( $field: fact(lookup($var)), )+ }
            }

            /// The facts that are present, as `(variable, value)` in allowlist order.
            pub fn present(&self) -> Vec<(&'static str, &str)> {
                let mut facts = Vec::new();
                $( if let Some(value) = &self.$field { facts.push(($var, value.as_str())); } )+
                facts
            }
        }
    };
}

platform_facts! {
    /// `FLY_APP_NAME`: the application the Machine belongs to.
    fly_app_name = "FLY_APP_NAME",
    /// `FLY_REGION`: the region the Machine runs in.
    fly_region = "FLY_REGION",
    /// `PRIMARY_REGION`: the region the application declares as its primary.
    primary_region = "PRIMARY_REGION",
    /// `FLY_MACHINE_ID`: the Machine's own id.
    fly_machine_id = "FLY_MACHINE_ID",
    /// `FLY_MACHINE_VERSION`: the version of the Machine's configuration.
    fly_machine_version = "FLY_MACHINE_VERSION",
    /// `FLY_IMAGE_REF`: the image the Machine was started from, which is what says that
    /// the deployed image is the one that was built.
    fly_image_ref = "FLY_IMAGE_REF",
    /// `FLY_PROCESS_GROUP`: the process group the Machine runs.
    fly_process_group = "FLY_PROCESS_GROUP",
    /// `FLY_VM_MEMORY_MB`: the memory the Machine was given, in megabytes.
    fly_vm_memory_mb = "FLY_VM_MEMORY_MB",
}

impl Platform {
    /// The facts of the platform this process runs on, read from its environment.
    ///
    /// ```
    /// use majordomus_cli::platform::{Platform, ALLOWLIST};
    ///
    /// // whatever this process was started with, the answer is the allowlist's fields
    /// let platform = Platform::from_env();
    /// let fields = serde_json::to_value(&platform).unwrap();
    /// assert_eq!(fields.as_object().unwrap().len(), ALLOWLIST.len());
    /// // and every fact it reports was read from an allowlisted name
    /// assert!(platform.present().iter().all(|(name, _)| ALLOWLIST.contains(name)));
    /// ```
    pub fn from_env() -> Self {
        Self::read(|name| std::env::var(name).ok())
    }
}

/// One value as the answer carries it: trimmed, and absent when nothing is left.
fn fact(value: Option<String>) -> Option<String> {
    value
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A value for `name` that says whose it is, so that a leak names the variable it came
    /// from. Assembled at run time: no committed file carries a credential's shape.
    fn sentinel(name: &str) -> String {
        format!("value-of-{name}-{}", "x".repeat(8))
    }

    /// Names that look like what a platform injects beside its facts, and must never leave.
    const OUTSIDE: &[&str] = &[
        "FLY_API_TOKEN",
        "FLY_ACCESS_TOKEN",
        "FLY_PUBLIC_IP",
        "FLY_PRIVATE_IP",
        "DATABASE_URL",
        "SECRET_KEY_BASE",
        "SESSION_SECRET",
        "MAJORDOMUS_SECRET",
        "AWS_SECRET_ACCESS_KEY",
        "GITHUB_TOKEN",
        "HOME",
        "PATH",
    ];

    #[test]
    fn only_the_allowlisted_names_are_asked_for_once_each_in_order() {
        let mut asked = Vec::new();
        Platform::read(|name| {
            asked.push(name.to_string());
            None
        });
        assert_eq!(asked, ALLOWLIST);
        for name in OUTSIDE {
            assert!(!ALLOWLIST.contains(name), "{name} is on the allowlist");
        }
    }

    #[test]
    fn a_variable_outside_the_allowlist_never_reaches_the_answer() {
        // an environment where every name is set, the secret-shaped ones included
        let platform = Platform::read(|name| Some(sentinel(name)));
        let text = serde_json::to_string(&platform).unwrap();
        for name in OUTSIDE {
            assert!(!text.contains(&sentinel(name)), "{name} leaked: {text}");
            assert!(
                !text.contains(name),
                "{name} is named in the answer: {text}"
            );
        }
        // and every allowlisted one does, under its own field
        for name in ALLOWLIST {
            assert!(text.contains(&sentinel(name)), "{name} missing: {text}");
        }
        let present: Vec<&str> = platform.present().into_iter().map(|(n, _)| n).collect();
        assert_eq!(present, ALLOWLIST);
    }

    #[test]
    fn the_answer_has_one_field_per_allowlisted_name_and_no_other() {
        let v = serde_json::to_value(Platform::read(|name| Some(sentinel(name)))).unwrap();
        let fields: Vec<String> = v.as_object().unwrap().keys().cloned().collect();
        let expected: Vec<String> = ALLOWLIST.iter().map(|n| n.to_lowercase()).collect();
        assert_eq!(fields, expected);
        for (field, value) in v.as_object().unwrap() {
            assert_eq!(value, &serde_json::json!(sentinel(&field.to_uppercase())));
        }
    }

    #[test]
    fn absence_is_null_not_an_error() {
        let bare = Platform::read(|_| None);
        let v = serde_json::to_value(&bare).unwrap();
        assert_eq!(v.as_object().unwrap().len(), ALLOWLIST.len());
        assert!(v.as_object().unwrap().values().all(|x| x.is_null()), "{v}");
        assert!(bare.present().is_empty());
    }

    #[test]
    fn an_empty_or_blank_value_is_absent_and_a_value_is_trimmed() {
        let platform = Platform::read(|name| match name {
            "FLY_REGION" => Some("  ams\n".into()),
            "FLY_APP_NAME" => Some("   ".into()),
            "FLY_MACHINE_ID" => Some(String::new()),
            _ => None,
        });
        assert_eq!(platform.fly_region.as_deref(), Some("ams"));
        assert_eq!(platform.fly_app_name, None);
        assert_eq!(platform.fly_machine_id, None);
        assert_eq!(platform.present(), vec![("FLY_REGION", "ams")]);
    }
}
