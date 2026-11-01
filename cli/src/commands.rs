//! Shared command namespace names for file and environment adapters.

/// Namespace names and their environment prefixes, longest prefixes first.
const COMMANDS: &[(&str, &str)] = &[
    ("REWRITE_EXPLAIN_", "rewrite_explain"),
    ("REWRITE_CHECK_", "rewrite_check"),
    ("REWRITE_APPLY_", "rewrite_apply"),
    ("REWRITE_DIFF_", "rewrite_diff"),
    ("REWRITE_", "rewrite"),
    ("INSTALL_", "install"),
    ("DOCTOR_", "doctor"),
    ("CHECK_", "check"),
    ("LS_", "ls"),
];

/// Check whether a configuration namespace names a planned command.
pub(crate) fn is_command(name: &str) -> bool {
    COMMANDS.iter().any(|(_, command)| *command == name)
}

/// Split a command-specific environment key into its namespace and field.
pub(crate) fn environment_key(key: &str) -> Option<(&'static str, &str)> {
    let rest = key.strip_prefix("WHITAKER_CMDS_")?;
    COMMANDS
        .iter()
        .find_map(|(prefix, command)| rest.strip_prefix(prefix).map(|setting| (*command, setting)))
}
