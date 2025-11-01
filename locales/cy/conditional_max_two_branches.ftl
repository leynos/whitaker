## Cadwch amodau’n fyr.

conditional_max_two_branches =
    Symleiddiwch { $name } i ddwy gangen neu lai.
    .note =
        { $branches ->
            [zero] Ar hyn o bryd mae { $branches } canghennau yn y rheol.
            [one] Ar hyn o bryd mae { $branches } gangen yn y rheol.
            [two] Ar hyn o bryd mae { $branches } gangen yn y rheol.
            [few] Ar hyn o bryd mae { $branches } changen yn y rheol.
            [many] Ar hyn o bryd mae { $branches } changen yn y rheol.
            *[other] Ar hyn o bryd mae { $branches } canghennau yn y rheol.
        }
    .help = Tynnwch god cymorth neu ailstrwythurwch { $name } i ostwng y canghennau.
