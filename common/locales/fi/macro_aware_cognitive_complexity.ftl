## Kirjoitetun koodin kognitiivisen kompleksisuuden tulee pysyä määritetyn rajan sisällä.

# `name`, `score`, and `limit` are rendered by the lint.
macro_aware_cognitive_complexity = Kohteen `{ $name }` kognitiivinen kompleksisuus on { $score } (raja { $limit }).
    .note = Makron laajennuksen tuottamaa haarautumista ei lasketa.
    .help = Jaa funktio pienempiin, selkeästi nimettyihin funktioihin.
