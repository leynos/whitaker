//! Property tests for shared `rstest` fingerprint data models.

use crate::rstest::{ArgAtom, ArgFingerprint, LocalSlot, ParagraphNormalizer};
use proptest::prelude::*;

proptest! {
    /// Slot indices are assigned in strict first-appearance order: the first
    /// distinct name always receives slot 0, the second slot 1, etc.
    #[test]
    fn paragraph_normalizer_assigns_slots_in_first_appearance_order(
        names in prop::collection::vec("[a-z]{1,16}", 1..=32_usize)
    ) {
        let mut norm = ParagraphNormalizer::new();
        let mut seen: Vec<String> = Vec::new();
        for name in &names {
            let slot = norm.local_slot(name.as_str());
            if !seen.contains(name) {
                let expected = seen.len() as u32;
                prop_assert_eq!(
                    slot.index(),
                    expected,
                    "first appearance of {:?} expected slot {}, got {}",
                    name,
                    expected,
                    slot.index()
                );
                seen.push(name.clone());
            }
        }
    }

    /// Calling `local_slot` twice with the same name returns the same slot.
    #[test]
    fn paragraph_normalizer_is_idempotent_for_same_name(
        name in "[a-z]{1,16}",
        prefix in prop::collection::vec("[a-z]{1,16}", 0..=8_usize)
    ) {
        let mut norm = ParagraphNormalizer::new();
        for p in &prefix {
            let _ = norm.local_slot(p.as_str());
        }
        let first = norm.local_slot(name.as_str());
        let second = norm.local_slot(name.as_str());
        prop_assert_eq!(first, second);
    }

    /// Two `ArgFingerprint` values built from equal atom sequences must compare equal.
    #[test]
    fn argument_fingerprint_equality_holds_for_equal_sequences(
        texts in prop::collection::vec("[a-z]{1,16}", 0..=16_usize)
    ) {
        let atoms: Vec<ArgAtom> = texts
            .iter()
            .map(|t| ArgAtom::fixture_local(t.as_str()))
            .collect();
        let fp1 = ArgFingerprint::new(atoms.clone());
        let fp2 = ArgFingerprint::new(atoms);
        prop_assert_eq!(fp1, fp2);
    }

    /// A `LocalSlot` always roundtrips its index value.
    #[test]
    fn local_slot_roundtrips_index(index in 0_u32..=u32::MAX) {
        prop_assert_eq!(LocalSlot::new(index).index(), index);
    }
}
