#![deny(no_std_fs_operations)]

mod capability_fs {
    #[derive(Default)]
    pub struct Dir;

    impl Dir {
        pub fn open(&self, _path: &Utf8PathBuf) -> Utf8PathBuf {
            _path.clone()
        }
    }

    #[derive(Clone)]
    pub struct Utf8PathBuf(String);

    impl Utf8PathBuf {
        pub fn from(input: &str) -> Self {
            Self(input.to_owned())
        }

        pub fn as_str(&self) -> &str {
            &self.0
        }
    }
}

use capability_fs::{Dir, Utf8PathBuf};

pub struct CapabilityBundle<'a> {
    pub dir: &'a Dir,
    pub target: &'a Utf8PathBuf,
}

pub fn plan_copy(bundle: CapabilityBundle<'_>) -> (&Dir, &Utf8PathBuf) {
    (bundle.dir, bundle.target)
}

fn main() {
    let dir = Dir::default();
    let rel = Utf8PathBuf::from("Cargo.toml");
    let _ = plan_copy(CapabilityBundle { dir: &dir, target: &rel });
    let _ = dir.open(&rel).as_str();
}
