## Conditionals should remain shallow.

# `branch_phrase` should render the count and noun (e.g. "3 branches").
conditional_max_n_branches = Conditional has { $branches } predicate atoms which exceeds the { $limit } branch limit.
    .note = Complex conditionals hinder readability and contribute to the Complex Method smell.
    .help = Extract the conditional to a well-named function or bind it to a local variable.
