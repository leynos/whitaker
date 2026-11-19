## Kompleksitas kognitif kode yang ditulis sendiri sebaiknya tetap dalam batas yang dikonfigurasi.

# `name`, `score`, and `limit` are rendered by the lint.
macro_aware_cognitive_complexity = `{ $name }` memiliki kompleksitas kognitif { $score } (batas { $limit }).
    .note = Percabangan yang dihasilkan oleh ekspansi makro tidak dihitung.
    .help = Pecah fungsi tersebut menjadi fungsi-fungsi yang lebih kecil dengan nama yang jelas.
