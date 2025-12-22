## Bumpy Road complexity detector.

bumpy_road_function = Multiple clusters of nested conditional logic in `{ $name }`.
    .note = Detected { $count } complexity { $count ->
        [one] bump
       *[other] bumps
    } above the threshold { $threshold }.
    .help = Extract helper functions from the highlighted regions to reduce clustered complexity.
    .label = Complexity bump { $index } spans { $lines } { $lines ->
        [one] line
       *[other] lines
    }.
