## เงื่อนไขควรตื้นอยู่เสมอ

# `branches` and `limit` are numbers; select on them for plural agreement.
conditional_max_n_branches = ลด “{ $name }” ให้เหลือไม่เกิน { $limit } สาขา
    .note = ขณะนี้ “{ $name }” มี { $branches } สาขา
    .help = แยกฟังก์ชันตัวช่วยออกมา หรือทำให้ “{ $name }” เรียบง่ายขึ้นเพื่อลดการแตกสาขา
