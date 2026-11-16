## يجب أن تبقى التعبيرات الشرطية قليلة العمق.

# `branches` and `limit` are numbers; select on them for plural agreement.
conditional_max_n_branches = { $limit ->
        [zero] اختزل «{ $name }» بحيث لا يبقى فيه أي فرع.
        [one] اختزل «{ $name }» إلى فرع واحد على الأكثر.
        [two] اختزل «{ $name }» إلى فرعين على الأكثر.
        [few] اختزل «{ $name }» إلى { $limit } فروع على الأكثر.
        [many] اختزل «{ $name }» إلى { $limit } فرعًا على الأكثر.
       *[other] اختزل «{ $name }» إلى { $limit } فرع على الأكثر.
    }
    .note = { $branches ->
        [zero] لا يحتوي «{ $name }» حاليًا على أي فرع.
        [one] يحتوي «{ $name }» حاليًا على فرع واحد.
        [two] يحتوي «{ $name }» حاليًا على فرعين.
        [few] يحتوي «{ $name }» حاليًا على { $branches } فروع.
        [many] يحتوي «{ $name }» حاليًا على { $branches } فرعًا.
       *[other] يحتوي «{ $name }» حاليًا على { $branches } فرع.
    }
    .help = استخرج دوال مساعدة أو بسّط «{ $name }» لتقليل التفرع.
