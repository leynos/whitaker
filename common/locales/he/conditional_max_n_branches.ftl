## ביטויי תנאי צריכים להישאר שטוחים.

# `branches` and `limit` are numbers; select on them for plural agreement.
conditional_max_n_branches = { $limit ->
        [one] יש לצמצם את "{ $name }" לענף אחד לכל היותר.
        [two] יש לצמצם את "{ $name }" לשני ענפים לכל היותר.
        [many] יש לצמצם את "{ $name }" ל-{ $limit } ענפים לכל היותר.
       *[other] יש לצמצם את "{ $name }" ל-{ $limit } ענפים לכל היותר.
    }
    .note = { $branches ->
        [one] "{ $name }" מכיל כעת ענף אחד.
        [two] "{ $name }" מכיל כעת שני ענפים.
        [many] "{ $name }" מכיל כעת { $branches } ענפים.
       *[other] "{ $name }" מכיל כעת { $branches } ענפים.
    }
    .help = יש לחלץ פונקציות עזר או לפשט את "{ $name }" כדי להפחית את ההסתעפות.
