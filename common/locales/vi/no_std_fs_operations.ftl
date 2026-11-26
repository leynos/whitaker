## Hạn chế các thao tác std::fs để áp dụng I/O dựa trên năng lực.

no_std_fs_operations = Thao tác std::fs `{ $operation }` bỏ qua chính sách hệ thống tệp dựa trên năng lực.
    .note = std::fs chạm tới thư mục làm việc ngầm định; thay vào đó hãy nhận các handle `cap_std::fs::Dir` và đường dẫn camino để bên gọi chọn phạm vi năng lực.
    .help = Hãy truyền `cap_std::fs::Dir` cùng các tham số `camino::Utf8Path`/`Utf8PathBuf` qua API thay vì gọi trực tiếp std::fs.

no_std_fs_operations_unknown_keys = Đang bỏ qua các khóa không xác định { $keys } trong [no_std_fs_operations] cho no_std_fs_operations { $version } (bản sửa đổi { $revision }).
    .note = Các khóa khác trong phần này vẫn được áp dụng; các khóa không được nhận diện không có tác dụng.
    .help = Cập nhật bộ Whitaker lên bản sửa đổi hiểu các khóa này, hoặc xóa chúng khỏi dylint.toml.

no_std_fs_operations_discarded_section = Đang loại bỏ cấu hình [no_std_fs_operations] cho no_std_fs_operations { $version } (bản sửa đổi { $revision }): { $error }
    .note = Không có loại trừ nào được áp dụng; lint đang dùng cài đặt mặc định.
    .help = Sửa giá trị không hợp lệ trong dylint.toml, hoặc cập nhật bộ Whitaker để hiểu khóa này.
