## Panik oluşturan unwrap_or_else yedeklerinden kaçınılmalıdır.

no_unwrap_or_else_panic = { $receiver } üzerindeki unwrap_or_else çağrısını panik oluşturmayan bir yedekle değiştirin.
    .note = unwrap_or_else çağrısına verilen kapanış bir panik tetikliyor.
    .help = Panik oluşturmak yerine hatayı yukarı iletin veya açıklayıcı bir iletiyle `expect` kullanın.
