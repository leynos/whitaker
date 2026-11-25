## Độ phức tạp nhận thức của mã tự viết nên nằm trong giới hạn đã cấu hình.

# `name`, `score`, and `limit` are rendered by the lint.
macro_aware_cognitive_complexity = `{ $name }` có độ phức tạp nhận thức là { $score } (giới hạn { $limit }).
    .note = Không tính việc rẽ nhánh do khai triển macro sinh ra.
    .help = Hãy chia hàm thành các hàm nhỏ hơn có tên rõ ràng.
