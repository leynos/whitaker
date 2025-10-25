## Tests must avoid example sections in documentation.

test_must_not_have_example = Remove example sections from test { $test } documentation.
    .note = The docs for { $test } contain an `# Examples` section.
    .help = Drop the example or move it into standalone user-facing documentation.
