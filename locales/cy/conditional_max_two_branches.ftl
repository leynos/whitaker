## Cadwch amodau’n fyr.

conditional_max_two_branches = Symleiddiwch { $name } i ddwy gangen neu lai.
    .note = Ar hyn o bryd mae { $branches ->
        [zero] dim cangen
        [one] { $branches } cangen
        [two] dwy gangen
        [few] { $branches } changen
        [many] { $branches } changen
       *[other] { $branches } cangen
    } yn y rheol.
    .help = Tynnwch god cymorth neu ailstrwythurwch { $name } i ostwng y canghennau.
