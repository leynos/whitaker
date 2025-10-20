## Restrict expect calls outside test contexts.

no_expect_outside_tests = Avoid calling expect on { $receiver } outside test-only code.
    .note = The call originates within { $context } which is not recognised as a test.
    .help = Handle the error returned by { $receiver } or move the code into a test.
