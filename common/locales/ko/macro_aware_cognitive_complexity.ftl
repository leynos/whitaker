## 직접 작성한 코드의 인지 복잡도는 설정된 한도 안에 있어야 합니다.

# `name`, `score`, and `limit` are rendered by the lint.
macro_aware_cognitive_complexity = `{ $name }`의 인지 복잡도는 { $score }입니다(한도 { $limit }).
    .note = 매크로 확장으로 생성된 분기는 계산하지 않습니다.
    .help = 함수를 이름이 명확한 더 작은 함수들로 나누십시오.
