//! Deterministic bounded-state coverage for the example-harness lock protocol.
//!
//! Each generated schedule starts with acquire attempts by both owners and adds
//! at most 24 transitions. The model has no clock, threads, filesystem, or OS
//! advisory locks; `MarkStale` supplies age eligibility directly. After every
//! transition it checks that there is at most one liveness owner, stale recovery
//! never removes a live owner's directory, owner-aware cleanup removes only a
//! matching owner, failed cleanup does not transfer ownership, and liveness is
//! released only after cleanup is attempted. Dedicated cases cover reclamation
//! after release and deterministic replay of every generated schedule.

use proptest::prelude::*;

const MAX_SCHEDULED_OPERATIONS: usize = 24;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Owner {
    First,
    Second,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LockOperation {
    Acquire(Owner),
    MarkStale,
    RecoverStale,
    ReplaceOwner(Owner),
    RemoveOwnerMetadata,
    RemoveDirectory,
    FailOwnerFileRemoval,
    FailDirectoryRemoval,
    Drop(Owner),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Directory {
    owner: Option<Owner>,
    is_stale: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct CleanupFailures {
    owner_file_removal: bool,
    directory_removal: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct LockModel {
    directory: Option<Directory>,
    liveness_owner: Option<Owner>,
    cleanup_attempted: [bool; 2],
    last_release_followed_cleanup: bool,
    last_owner_aware_removal: Option<(Owner, Owner)>,
    last_cleanup_failed: bool,
    cleanup_failures: CleanupFailures,
}

impl Default for LockModel {
    fn default() -> Self {
        Self {
            directory: None,
            liveness_owner: None,
            cleanup_attempted: [false; 2],
            last_release_followed_cleanup: true,
            last_owner_aware_removal: None,
            last_cleanup_failed: false,
            cleanup_failures: CleanupFailures::default(),
        }
    }
}

impl LockModel {
    fn apply(&mut self, operation: LockOperation) {
        self.last_owner_aware_removal = None;
        self.last_cleanup_failed = false;
        match operation {
            LockOperation::Acquire(owner) => self.acquire(owner),
            LockOperation::MarkStale => self.mark_stale(),
            LockOperation::RecoverStale => self.recover_stale(),
            LockOperation::ReplaceOwner(owner) => self.replace_owner(owner),
            LockOperation::RemoveOwnerMetadata => self.remove_owner_metadata(),
            LockOperation::RemoveDirectory => self.directory = None,
            LockOperation::FailOwnerFileRemoval => {
                self.cleanup_failures.owner_file_removal = true;
            }
            LockOperation::FailDirectoryRemoval => {
                self.cleanup_failures.directory_removal = true;
            }
            LockOperation::Drop(owner) => self.drop_owner(owner),
        }
    }

    fn acquire(&mut self, owner: Owner) {
        if self.liveness_owner.is_none() && self.directory.is_none() {
            self.liveness_owner = Some(owner);
            self.directory = Some(Directory {
                owner: Some(owner),
                is_stale: false,
            });
        }
    }

    fn mark_stale(&mut self) {
        if let Some(directory) = &mut self.directory {
            directory.is_stale = true;
        }
    }

    fn recover_stale(&mut self) {
        if self.liveness_owner.is_none()
            && self
                .directory
                .as_ref()
                .is_some_and(|directory| directory.is_stale)
        {
            self.remove_directory(None);
        }
    }

    fn replace_owner(&mut self, owner: Owner) {
        if let Some(directory) = &mut self.directory {
            directory.owner = Some(owner);
        }
    }

    fn remove_owner_metadata(&mut self) {
        if let Some(directory) = &mut self.directory {
            directory.owner = None;
        }
    }

    fn drop_owner(&mut self, owner: Owner) {
        if self.liveness_owner != Some(owner) {
            return;
        }

        self.cleanup_attempted[owner.index()] = true;
        if self
            .directory
            .as_ref()
            .and_then(|directory| directory.owner)
            == Some(owner)
        {
            self.remove_directory(Some(owner));
        }

        self.last_release_followed_cleanup = self.cleanup_attempted[owner.index()];
        self.liveness_owner = None;
    }

    fn remove_directory(&mut self, cleaner: Option<Owner>) {
        if self.cleanup_failures.owner_file_removal {
            self.cleanup_failures.owner_file_removal = false;
            self.last_cleanup_failed = true;
            return;
        }

        if self.cleanup_failures.directory_removal {
            self.cleanup_failures.directory_removal = false;
            self.last_cleanup_failed = true;
            if let Some(directory) = &mut self.directory {
                directory.owner = None;
            }
            return;
        }

        if let (Some(cleaner), Some(owner)) = (
            cleaner,
            self.directory
                .as_ref()
                .and_then(|directory| directory.owner),
        ) {
            self.last_owner_aware_removal = Some((cleaner, owner));
        }
        self.directory = None;
    }

    fn assert_step_invariants(
        &self,
        before: &Self,
        operation: LockOperation,
    ) -> Result<(), String> {
        self.assert_owner_aware_cleanup_keeps_owner()?;
        self.assert_failed_cleanup_preserves_liveness_owner(before)?;
        self.assert_stale_recovery_preserves_live_directory(before, operation)?;
        self.assert_liveness_release_follows_cleanup(before)
    }

    fn assert_owner_aware_cleanup_keeps_owner(&self) -> Result<(), String> {
        let Some((cleaner, owner)) = self.last_owner_aware_removal else {
            return Ok(());
        };
        if cleaner == owner {
            return Ok(());
        }

        Err("owner-aware cleanup removed a different owner".to_owned())
    }

    fn assert_failed_cleanup_preserves_liveness_owner(&self, before: &Self) -> Result<(), String> {
        if !self.last_cleanup_failed {
            return Ok(());
        }

        let Some(liveness_owner) = self.liveness_owner else {
            return Ok(());
        };
        if Some(liveness_owner) == before.liveness_owner {
            return Ok(());
        }

        Err("failed cleanup transferred liveness ownership".to_owned())
    }

    fn assert_stale_recovery_preserves_live_directory(
        &self,
        before: &Self,
        operation: LockOperation,
    ) -> Result<(), String> {
        if !matches!(operation, LockOperation::RecoverStale) {
            return Ok(());
        }
        if before.liveness_owner.is_none() {
            return Ok(());
        }
        if before.directory.is_none() {
            return Ok(());
        }
        if self.directory == before.directory {
            return Ok(());
        }

        Err("stale recovery removed a live owner's directory".to_owned())
    }

    fn assert_liveness_release_follows_cleanup(&self, before: &Self) -> Result<(), String> {
        if self.liveness_owner.is_some() {
            return Ok(());
        }
        if before.liveness_owner.is_none() {
            return Ok(());
        }
        if self.last_release_followed_cleanup {
            return Ok(());
        }

        Err("liveness released before owner-aware cleanup".to_owned())
    }
}

impl Owner {
    const fn index(self) -> usize {
        match self {
            Self::First => 0,
            Self::Second => 1,
        }
    }
}

fn operation_strategy() -> impl Strategy<Value = LockOperation> {
    prop_oneof![
        Just(LockOperation::Acquire(Owner::First)),
        Just(LockOperation::Acquire(Owner::Second)),
        Just(LockOperation::MarkStale),
        Just(LockOperation::RecoverStale),
        Just(LockOperation::ReplaceOwner(Owner::First)),
        Just(LockOperation::ReplaceOwner(Owner::Second)),
        Just(LockOperation::RemoveOwnerMetadata),
        Just(LockOperation::RemoveDirectory),
        Just(LockOperation::FailOwnerFileRemoval),
        Just(LockOperation::FailDirectoryRemoval),
        Just(LockOperation::Drop(Owner::First)),
        Just(LockOperation::Drop(Owner::Second)),
    ]
}

fn bounded_schedule_strategy() -> impl Strategy<Value = Vec<LockOperation>> {
    prop::collection::vec(operation_strategy(), 0..=MAX_SCHEDULED_OPERATIONS).prop_map(
        |mut operations| {
            operations.insert(0, LockOperation::Acquire(Owner::Second));
            operations.insert(0, LockOperation::Acquire(Owner::First));
            operations
        },
    )
}

fn replay(schedule: &[LockOperation]) -> Result<LockModel, String> {
    let mut model = LockModel::default();
    for operation in schedule {
        let before = model.clone();
        model.apply(*operation);
        model.assert_step_invariants(&before, *operation)?;
    }
    Ok(model)
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 256,
        .. ProptestConfig::default()
    })]

    #[test]
    fn bounded_lock_schedules_preserve_protocol_invariants(
        schedule in bounded_schedule_strategy(),
    ) {
        let first = replay(&schedule);
        let second = replay(&schedule);

        prop_assert!(first.is_ok(), "schedule violated an invariant: {first:?}");
        prop_assert_eq!(first, second);
    }
}

#[test]
fn stale_recovery_reclaims_only_after_liveness_release() {
    let schedule = [
        LockOperation::Acquire(Owner::First),
        LockOperation::MarkStale,
        LockOperation::RecoverStale,
        LockOperation::Drop(Owner::First),
        LockOperation::RecoverStale,
    ];

    let model = replay(&schedule).expect("model schedule should preserve invariants");
    assert!(model.directory.is_none());
    assert!(model.liveness_owner.is_none());
}

#[test]
fn cleanup_failure_keeps_ownership_until_liveness_releases() {
    let schedule = [
        LockOperation::Acquire(Owner::First),
        LockOperation::FailOwnerFileRemoval,
        LockOperation::Drop(Owner::First),
        LockOperation::Acquire(Owner::Second),
    ];

    let model = replay(&schedule).expect("model schedule should preserve invariants");
    assert_eq!(
        model.directory.and_then(|directory| directory.owner),
        Some(Owner::First)
    );
    assert!(model.liveness_owner.is_none());
}
