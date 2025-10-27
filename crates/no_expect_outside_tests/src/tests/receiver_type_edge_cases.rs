use super::{ContextLabel, Localiser, ReceiverLabel, localised_messages};

#[test]
fn handles_empty_receiver_type() {
    let lookup = Localiser::new(Some("en-GB"));
    let receiver = ReceiverLabel::new("");
    let context = ContextLabel::new("the surrounding scope");
    let messages = localised_messages(&lookup, &receiver, &context).expect("localisation succeeds");
    assert!(!messages.primary().is_empty());
}

#[test]
fn handles_malformed_receiver_type() {
    let lookup = Localiser::new(Some("en-GB"));
    let receiver = ReceiverLabel::new("!!!not_a_type");
    let context = ContextLabel::new("function `worker`");
    let messages = localised_messages(&lookup, &receiver, &context).expect("localisation succeeds");
    assert!(!messages.note().is_empty());
}

#[test]
fn handles_unexpected_receiver_type() {
    let lookup = Localiser::new(Some("en-GB"));
    let receiver = ReceiverLabel::new("SomeCompletelyUnexpectedType123");
    let context = ContextLabel::new("function `processor`");
    let messages = localised_messages(&lookup, &receiver, &context).expect("localisation succeeds");
    assert!(!messages.help().is_empty());
}
