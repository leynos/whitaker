## Restrict std::fs operations to enforce capability-based I/O.

no_std_fs_operations = std::fs operation `{ $operation }` bypasses the capability-based filesystem policy.
    .note = std::fs touches the ambient working directory; accept `cap_std::fs::Dir` handles and camino paths instead so callers choose the capability surface.
    .help = Pass `cap_std::fs::Dir` plus `camino::Utf8Path`/`Utf8PathBuf` parameters through your APIs instead of calling std::fs directly.
