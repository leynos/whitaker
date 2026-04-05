"""Test data constants for dependency binaries manifest tests."""

#: Valid manifest with two entries
VALID_MANIFEST = """\
[[dependency_binaries]]
package = "cargo-dylint"
binary = "cargo-dylint"
version = "4.1.0"
license = "MIT OR Apache-2.0"
repository = "https://github.com/trailofbits/dylint"

[[dependency_binaries]]
package = "dylint-link"
binary = "dylint-link"
version = "4.1.0"
license = "MIT OR Apache-2.0"
repository = "https://github.com/trailofbits/dylint"
"""

#: Single entry manifest for simple tests
SINGLE_ENTRY_MANIFEST = """\
[[dependency_binaries]]
package = "cargo-dylint"
binary = "cargo-dylint"
version = "4.1.0"
license = "MIT OR Apache-2.0"
repository = "https://github.com/trailofbits/dylint"
"""

#: Duplicate package manifest for error testing
DUPLICATE_MANIFEST = """\
[[dependency_binaries]]
package = "cargo-dylint"
binary = "cargo-dylint"
version = "4.1.0"
license = "MIT OR Apache-2.0"
repository = "https://github.com/trailofbits/dylint"

[[dependency_binaries]]
package = "cargo-dylint"
binary = "cargo-dylint"
version = "4.2.0"
license = "MIT OR Apache-2.0"
repository = "https://github.com/trailofbits/dylint"
"""
