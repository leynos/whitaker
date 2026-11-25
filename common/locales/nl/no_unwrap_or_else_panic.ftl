## Ontmoedig `unwrap_or_else`-fallbacks die een panic veroorzaken.

no_unwrap_or_else_panic = Vervang unwrap_or_else op { $receiver } door een fallback die geen panic veroorzaakt.
    .note = De closure die aan unwrap_or_else wordt meegegeven, veroorzaakt een panic.
    .help = Geef de fout door of gebruik `expect` met een beschrijvend bericht in plaats van een panic te veroorzaken.
