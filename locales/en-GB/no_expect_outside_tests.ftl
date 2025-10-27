## Restrict expect calls outside test contexts.

no_expect_outside_tests = Avoid calling expect on { $receiver } outside test-only code.
    .note = The call originates within { $context } which is not recognised as a test.
    .help = { $handling ->
        [option] Handle the `None` case for { $receiver } or move the code into a test.
        [result] Handle the `Err` variant of { $receiver } or move the code into a test.
       *[other] Handle the error path for { $receiver } or move the code into a test.
    }
