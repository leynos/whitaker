## ไม่สนับสนุนทางสำรองของ unwrap_or_else ที่ panic

no_unwrap_or_else_panic = แทนที่ `unwrap_or_else` บน { $receiver } ด้วยทางสำรองที่ไม่ panic
    .note = โคลเชอร์ที่ส่งให้ `unwrap_or_else` ทำให้เกิด panic
    .help = ส่งต่อข้อผิดพลาด หรือใช้ `expect` พร้อมข้อความที่อธิบายชัดเจนแทนการ panic
