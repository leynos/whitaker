## Restrict std::fs operations to enforce capability-based I/O.

no_std_fs_operations = std::fs operation `{ $operation }` bypasses the capability-based filesystem policy.
    .note = std::fs touches the ambient working directory; accept `cap_std::fs::Dir` handles and camino paths instead so callers choose the capability surface.
    .help = Pass `cap_std::fs::Dir` plus `camino::Utf8Path`/`Utf8PathBuf` parameters through your APIs instead of calling std::fs directly.

no_std_fs_operations_unknown_keys = Ignoring unknown key(s) { $keys } in [no_std_fs_operations] for no_std_fs_operations { $version } (revision { $revision }).
    .note = Other keys in this section still apply; the unrecognized key(s) have no effect.
    .help = Update the Whitaker suite to a revision that understands these keys, or remove them from dylint.toml.

no_std_fs_operations_discarded_section = Discarding the [no_std_fs_operations] configuration for no_std_fs_operations { $version } (revision { $revision }): { $error }
    .note = No exclusions apply; the lint is using its default settings.
    .help = Fix the invalid value in dylint.toml, or update the Whitaker suite so it understands the key.
