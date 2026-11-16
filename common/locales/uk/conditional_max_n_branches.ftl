## Умовні вирази мають залишатися неглибокими.

# `branches` and `limit` are numbers; select on them for plural agreement.
conditional_max_n_branches = { $limit ->
        [one] Скоротіть вираз «{ $name }» щонайбільше до { $limit } гілки.
        [few] Скоротіть вираз «{ $name }» щонайбільше до { $limit } гілок.
        [many] Скоротіть вираз «{ $name }» щонайбільше до { $limit } гілок.
       *[other] Скоротіть вираз «{ $name }» щонайбільше до { $limit } гілки.
    }
    .note = { $branches ->
        [one] Зараз вираз «{ $name }» містить { $branches } гілку.
        [few] Зараз вираз «{ $name }» містить { $branches } гілки.
        [many] Зараз вираз «{ $name }» містить { $branches } гілок.
       *[other] Зараз вираз «{ $name }» містить { $branches } гілки.
    }
    .help = Винесіть допоміжні функції або спростіть вираз «{ $name }», щоб зменшити розгалуження.
