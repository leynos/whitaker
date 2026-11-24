## จำกัดการดำเนินการ std::fs เพื่อบังคับใช้ I/O ที่อิงขีดความสามารถ

no_std_fs_operations = การดำเนินการ std::fs `{ $operation }` ข้ามนโยบายระบบไฟล์ที่อิงขีดความสามารถ
    .note = std::fs เข้าถึงไดเรกทอรีทำงานโดยปริยาย จึงควรรับแฮนเดิล `cap_std::fs::Dir` และพาธของ camino แทน เพื่อให้ผู้เรียกเลือกขอบเขตของขีดความสามารถได้
    .help = ส่งพารามิเตอร์ `cap_std::fs::Dir` พร้อม `camino::Utf8Path`/`Utf8PathBuf` ผ่าน API แทนการเรียก std::fs โดยตรง

no_std_fs_operations_unknown_keys = กำลังเพิกเฉยคีย์ที่ไม่รู้จัก { $keys } ใน [no_std_fs_operations] สำหรับ no_std_fs_operations { $version } (รุ่นแก้ไข { $revision })
    .note = คีย์อื่นในส่วนนี้ยังคงมีผล คีย์ที่ไม่รู้จักไม่มีผลใด ๆ
    .help = อัปเดตชุด Whitaker เป็นรุ่นแก้ไขที่เข้าใจคีย์เหล่านี้ หรือนำออกจาก dylint.toml

no_std_fs_operations_discarded_section = กำลังทิ้งการกำหนดค่า [no_std_fs_operations] สำหรับ no_std_fs_operations { $version } (รุ่นแก้ไข { $revision }): { $error }
    .note = ไม่มีข้อยกเว้นใด ๆ ถูกนำมาใช้ lint กำลังใช้การตั้งค่าเริ่มต้น
    .help = แก้ไขค่าที่ไม่ถูกต้องใน dylint.toml หรืออัปเดตชุด Whitaker ให้เข้าใจคีย์นี้
