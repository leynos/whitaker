## Cadwch amodau’n fyr.

# `branch_phrase` should contain the rendered count and the correctly mutated
# noun (e.g. "3 changen", "5 cangen").
# `limit_phrase` should match the configured limit (e.g. "2 gangen").
conditional_max_n_branches = Symleiddiwch { $name } i { $limit_phrase } neu lai.
    .note = Ar hyn o bryd mae { $branch_phrase } yn y rheol.
    .help = Tynnwch god cymorth neu ailstrwythurwch { $name } i ostwng y canghennau.
