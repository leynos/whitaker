# The message is rendered by ui/, ui-cy/, ui-gd/, and ui-fallback/.
string_continuation_style = Join the physical lines in this cooked string with `concat!()`.
    .note = The escaped newline removes the physical line break and following indentation, hiding the fragment boundary inside one literal.
    .help = Split the cooked string into `concat!()` arguments at each source-line join.
    .label = source-line join
    .suggestion = replace with a `concat!()` expression
