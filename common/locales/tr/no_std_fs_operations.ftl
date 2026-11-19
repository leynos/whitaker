## Yetenek tabanlı G/Ç’yi zorunlu kılmak için std::fs işlemleri kısıtlanır.

no_std_fs_operations = std::fs işlemi `{ $operation }`, yetenek tabanlı dosya sistemi ilkesini atlıyor.
    .note = std::fs, ortamdaki çalışma dizinine dokunur; yetenek yüzeyini çağıranların seçebilmesi için bunun yerine `cap_std::fs::Dir` tanıtıcılarını ve camino yollarını kabul edin.
    .help = std::fs işlevlerini doğrudan çağırmak yerine API’leriniz üzerinden `cap_std::fs::Dir` ile `camino::Utf8Path`/`Utf8PathBuf` parametrelerini geçirin.
