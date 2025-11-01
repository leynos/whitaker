//! Edge-case localisation tests covering unusual receiver labels.

use super::{
    ContextLabel, Localizer, NoExpectMessages, ReceiverCategory, ReceiverLabel, localised_messages,
};
use rstest::rstest;

#[rstest]
#[case("", "the surrounding scope", |messages: &NoExpectMessages| !messages.primary().is_empty())]
#[case(
    "!!!not_a_type",
    "function `worker`",
    |messages: &NoExpectMessages| !messages.note().is_empty(),
)]
#[case(
    "SomeCompletelyUnexpectedType123",
    "function `processor`",
    |messages: &NoExpectMessages| !messages.help().is_empty(),
)]
fn handles_receiver_type_edge_cases(
    #[case] receiver: &str,
    #[case] context: &str,
    #[case] assertion: fn(&NoExpectMessages) -> bool,
) {
    let lookup = Localizer::new(Some("en-GB"));
    let receiver_label = ReceiverLabel::new(receiver);
    let context_label = ContextLabel::new(context);
    let category = ReceiverCategory::for_label(&receiver_label);
    let messages = localised_messages(&lookup, &receiver_label, &context_label, category)
        .expect("localisation succeeds");
    assert!(
        assertion(&messages),
        "Edge case assertion failed for receiver: {receiver}"
    );
}
