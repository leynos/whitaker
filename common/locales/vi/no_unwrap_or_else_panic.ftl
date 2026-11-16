## Không khuyến khích phương án dự phòng unwrap_or_else gây panic.

no_unwrap_or_else_panic = Hãy thay `unwrap_or_else` trên { $receiver } bằng một phương án dự phòng không gây panic.
    .note = Closure truyền cho `unwrap_or_else` gây ra panic.
    .help = Hãy lan truyền lỗi hoặc dùng `expect` kèm thông báo mô tả rõ ràng thay vì gây panic.
