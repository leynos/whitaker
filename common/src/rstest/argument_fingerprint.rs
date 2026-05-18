//! Pure argument fingerprints for repeated `rstest` helper-call evidence.

/// A lowered argument value that participates in helper-call grouping.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ArgAtom {
    /// An argument supplied by an `rstest` fixture-local parameter.
    FixtureLocal { name: String },
    /// A stable literal argument, stored as canonical source text.
    ConstLit { text: String },
    /// A stable constant path, stored as a canonical definition path.
    ConstPath { def_path: String },
    /// A present argument shape that later lowering does not support.
    Unsupported,
}

impl ArgAtom {
    /// Builds a fixture-local argument atom.
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker_common::rstest::ArgAtom;
    ///
    /// let atom = ArgAtom::fixture_local("db");
    /// assert_eq!(atom, ArgAtom::FixtureLocal { name: "db".to_string() });
    /// ```
    #[must_use]
    pub fn fixture_local(name: impl Into<String>) -> Self {
        Self::FixtureLocal { name: name.into() }
    }

    /// Builds a stable literal argument atom.
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker_common::rstest::ArgAtom;
    ///
    /// let atom = ArgAtom::const_lit("42");
    /// assert_eq!(atom, ArgAtom::ConstLit { text: "42".to_string() });
    /// ```
    #[must_use]
    pub fn const_lit(text: impl Into<String>) -> Self {
        Self::ConstLit { text: text.into() }
    }

    /// Builds a stable constant-path argument atom.
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker_common::rstest::ArgAtom;
    ///
    /// let atom = ArgAtom::const_path("crate::defaults::TIMEOUT");
    /// assert_eq!(
    ///     atom,
    ///     ArgAtom::ConstPath {
    ///         def_path: "crate::defaults::TIMEOUT".to_string(),
    ///     },
    /// );
    /// ```
    #[must_use]
    pub fn const_path(def_path: impl Into<String>) -> Self {
        Self::ConstPath {
            def_path: def_path.into(),
        }
    }

    /// Builds an explicit unsupported argument atom.
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker_common::rstest::ArgAtom;
    ///
    /// assert_eq!(ArgAtom::unsupported(), ArgAtom::Unsupported);
    /// ```
    #[must_use]
    pub const fn unsupported() -> Self {
        Self::Unsupported
    }
}

/// A positional fingerprint for one helper-call argument list.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct ArgFingerprint {
    atoms: Vec<ArgAtom>,
}

impl ArgFingerprint {
    /// Builds a fingerprint from argument atoms in call-site order.
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker_common::rstest::{ArgAtom, ArgFingerprint};
    ///
    /// let fingerprint = ArgFingerprint::new([
    ///     ArgAtom::fixture_local("db"),
    ///     ArgAtom::const_lit("42"),
    /// ]);
    ///
    /// assert_eq!(fingerprint.atoms().len(), 2);
    /// ```
    #[must_use]
    pub fn new<I>(atoms: I) -> Self
    where
        I: IntoIterator<Item = ArgAtom>,
    {
        Self {
            atoms: atoms.into_iter().collect(),
        }
    }

    /// Returns the stored atoms in positional order.
    #[must_use]
    pub fn atoms(&self) -> &[ArgAtom] {
        &self.atoms
    }

    /// Consumes the fingerprint and returns the stored atoms.
    #[must_use]
    pub fn into_atoms(self) -> Vec<ArgAtom> {
        self.atoms
    }
}
