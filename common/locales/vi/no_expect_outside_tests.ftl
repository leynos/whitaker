## Hạn chế lời gọi expect bên ngoài ngữ cảnh kiểm thử.

no_expect_outside_tests = Tránh gọi `expect` trên { $receiver } bên ngoài mã chỉ dành cho kiểm thử.
    .note = Lời gọi nằm trong “{ $context }”, vốn không được nhận diện là kiểm thử.
    .help = { $handling ->
        [option] Hãy xử lý biến thể `None` của { $receiver } hoặc chuyển mã vào một kiểm thử.
        [result] Hãy xử lý biến thể `Err` của { $receiver } hoặc chuyển mã vào một kiểm thử.
       *[other] Hãy xử lý luồng lỗi của { $receiver } hoặc chuyển mã vào một kiểm thử.
    }
