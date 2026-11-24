//! How the prepared `PATH` reaches the child command.

use super::*;

/// Deliver the prepared `PATH` to the child, and nothing else about it.
///
/// The value is injected through the prepared run, so the test never touches
/// the process environment.
#[test]
fn the_prepared_path_reaches_the_child_command() {
    let installed = Installed::new().expect("build the installation");
    let cargo = installed.cargo(true).expect("write stub cargo");
    let mut prepared =
        prepare(&installed.layout, cargo.as_os_str(), RunOptions::default()).expect("prepare");
    prepared.child_path = Some(OsString::from("/injected/bin"));

    let command = child_command(&prepared, cargo.as_os_str(), &[]);

    let path = command
        .get_envs()
        .find(|(name, _)| *name == OsStr::new("PATH"))
        .map(|(_, value)| value);
    assert_eq!(
        path,
        Some(Some(OsStr::new("/injected/bin"))),
        "the child must receive the PATH the prepared run carries"
    );
}

/// Leave the inherited `PATH` untouched when the run prepared no replacement.
#[test]
fn an_unprepared_path_is_left_inherited() {
    let installed = Installed::new().expect("build the installation");
    let cargo = installed.cargo(true).expect("write stub cargo");
    let options = RunOptions {
        inherited_path: Some(OsStr::new("/inherited/bin")),
        ..RunOptions::default()
    };
    let prepared = prepare(&installed.layout, cargo.as_os_str(), options).expect("prepare");

    let command = child_command(&prepared, cargo.as_os_str(), &[]);

    assert!(
        command
            .get_envs()
            .all(|(name, _)| name != OsStr::new("PATH")),
        "off Windows the child's PATH must stay exactly as inherited"
    );
}
