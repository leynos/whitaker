## Conditionals should remain shallow.

conditional_max_two_branches = Collapse conditional { $name } to two branches or fewer.
    .note = The conditional currently declares { $branches ->
        [one] { $branches } branch
       *[other] { $branches } branches
    }.
    .help = Extract helper functions or simplify { $name } to reduce branching.
