## Conditionals should remain shallow.

# `branch_phrase` should render the count and noun (e.g. "3 branches").
# `limit_phrase` should match the configured branch limit (e.g. "2 branches").
conditional_max_n_branches = Collapse the { $name } to { $limit_phrase } or fewer.
    .note = The { $name } currently contains { $branch_phrase }.
    .help = Extract helper functions or simplify the { $name } to reduce branching.
