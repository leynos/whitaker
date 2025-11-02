## Conditionals should remain shallow.

# `branch_phrase` should render the count and noun (e.g. "3 branches").
conditional_max_two_branches = Collapse conditional { $name } to two branches or fewer.
    .note = The conditional currently declares { $branch_phrase }.
    .help = Extract helper functions or simplify { $name } to reduce branching.
