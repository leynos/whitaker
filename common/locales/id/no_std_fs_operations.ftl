## Batasi operasi std::fs untuk menegakkan I/O berbasis kapabilitas.

no_std_fs_operations = Operasi std::fs `{ $operation }` melewati kebijakan sistem berkas berbasis kapabilitas.
    .note = std::fs menyentuh direktori kerja ambien; sebagai gantinya, terima handle `cap_std::fs::Dir` dan path camino agar pemanggil memilih cakupan kapabilitasnya.
    .help = Teruskan parameter `cap_std::fs::Dir` beserta `camino::Utf8Path`/`Utf8PathBuf` melalui API alih-alih memanggil std::fs secara langsung.
