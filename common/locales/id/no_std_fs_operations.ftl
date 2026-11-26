## Batasi operasi std::fs untuk menegakkan I/O berbasis kapabilitas.

no_std_fs_operations = Operasi std::fs `{ $operation }` melewati kebijakan sistem berkas berbasis kapabilitas.
    .note = std::fs menyentuh direktori kerja ambien; sebagai gantinya, terima handle `cap_std::fs::Dir` dan path camino agar pemanggil memilih cakupan kapabilitasnya.
    .help = Teruskan parameter `cap_std::fs::Dir` beserta `camino::Utf8Path`/`Utf8PathBuf` melalui API alih-alih memanggil std::fs secara langsung.

no_std_fs_operations_unknown_keys = Mengabaikan kunci tak dikenal { $keys } di [no_std_fs_operations] untuk no_std_fs_operations { $version } (revisi { $revision }).
    .note = Kunci lain di bagian ini tetap berlaku; kunci yang tidak dikenali tidak berpengaruh.
    .help = Perbarui rangkaian Whitaker ke revisi yang memahami kunci ini, atau hapus kunci tersebut dari dylint.toml.

no_std_fs_operations_discarded_section = Membuang konfigurasi [no_std_fs_operations] untuk no_std_fs_operations { $version } (revisi { $revision }): { $error }
    .note = Tidak ada pengecualian yang berlaku; lint menggunakan pengaturan bawaannya.
    .help = Perbaiki nilai yang tidak valid di dylint.toml, atau perbarui rangkaian Whitaker agar memahami kunci tersebut.
