## Module length guidance.

module_max_lines = Module { $module } spans { $lines } lines, exceeding the allowed { $limit }.
    .note = Large modules are harder to navigate and review.
    .help = Split { $module } into smaller modules or reduce its responsibilities.
