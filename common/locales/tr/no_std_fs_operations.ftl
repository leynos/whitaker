## Yetenek tabanlı G/Ç’yi zorunlu kılmak için std::fs işlemleri kısıtlanır.

no_std_fs_operations = std::fs işlemi `{ $operation }`, yetenek tabanlı dosya sistemi ilkesini atlıyor.
    .note = std::fs, ortamdaki çalışma dizinine dokunur; yetenek yüzeyini çağıranların seçebilmesi için bunun yerine `cap_std::fs::Dir` tanıtıcılarını ve camino yollarını kabul edin.
    .help = std::fs işlevlerini doğrudan çağırmak yerine API’leriniz üzerinden `cap_std::fs::Dir` ile `camino::Utf8Path`/`Utf8PathBuf` parametrelerini geçirin.

no_std_fs_operations_unknown_keys = no_std_fs_operations { $version } (revizyon { $revision }) için [no_std_fs_operations] içindeki bilinmeyen { $keys } anahtarları yok sayılıyor.
    .note = Bu bölümdeki diğer anahtarlar geçerliliğini korur; tanınmayan anahtarların hiçbir etkisi yoktur.
    .help = Whitaker paketini bu anahtarları anlayan bir revizyona güncelleyin veya bunları dylint.toml dosyasından kaldırın.

no_std_fs_operations_discarded_section = no_std_fs_operations { $version } (revizyon { $revision }) için [no_std_fs_operations] yapılandırması atılıyor: { $error }
    .note = Hiçbir dışlama uygulanmıyor; lint varsayılan ayarlarını kullanıyor.
    .help = dylint.toml içindeki geçersiz değeri düzeltin veya Whitaker paketini anahtarı anlayacak şekilde güncelleyin.
