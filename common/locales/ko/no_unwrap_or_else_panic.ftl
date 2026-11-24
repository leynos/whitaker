## 패닉하는 unwrap_or_else 폴백을 지양합니다.

no_unwrap_or_else_panic = { $receiver }에 대한 `unwrap_or_else`를 패닉하지 않는 폴백으로 바꾸십시오.
    .note = `unwrap_or_else`에 전달된 클로저가 패닉을 일으킵니다.
    .help = 패닉하는 대신 오류를 전파하거나 설명이 담긴 메시지와 함께 `expect`를 사용하십시오.
