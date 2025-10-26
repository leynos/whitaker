## Module length guidance.

module_max_400_lines = Module { $module } spans { $lines } lines which exceeds the { $limit } line limit.
    .note = Large modules are harder to navigate and review.
    .help = Split { $module } into smaller modules or reduce its responsibilities.
