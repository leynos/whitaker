## Hạn chế các thao tác std::fs để áp dụng I/O dựa trên năng lực.

no_std_fs_operations = Thao tác std::fs `{ $operation }` bỏ qua chính sách hệ thống tệp dựa trên năng lực.
    .note = std::fs chạm tới thư mục làm việc ngầm định; thay vào đó hãy nhận các handle `cap_std::fs::Dir` và đường dẫn camino để bên gọi chọn phạm vi năng lực.
    .help = Hãy truyền `cap_std::fs::Dir` cùng các tham số `camino::Utf8Path`/`Utf8PathBuf` qua API thay vì gọi trực tiếp std::fs.
