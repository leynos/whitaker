## Cadwch amodau’n fyr.

-branches-count =
    { $branches ->
        [zero] dim cangen
        [one] { $branches } cangen
        [two] dwy gangen
        [few] { $branches } changen
        [many] { $branches } changen
       *[other] { $branches } cangen
    }

conditional_max_two_branches = Symleiddiwch { $name } i ddwy gangen neu lai.
    .note = Ar hyn o bryd mae { -branches-count(branches: $branches) } yn y rheol.
    .help = Tynnwch god cymorth neu ailstrwythurwch { $name } i ostwng y canghennau.
