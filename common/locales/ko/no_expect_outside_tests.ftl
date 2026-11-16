## 테스트 컨텍스트 밖에서 expect 호출을 제한합니다.

no_expect_outside_tests = 테스트 전용 코드 밖에서는 { $receiver }에 `expect`를 호출하지 마십시오.
    .note = 이 호출은 테스트로 인식되지 않는 “{ $context }” 안에서 이루어집니다.
    .help = { $handling ->
        [option] { $receiver }의 `None` 배리언트를 처리하거나 코드를 테스트로 옮기십시오.
        [result] { $receiver }의 `Err` 배리언트를 처리하거나 코드를 테스트로 옮기십시오.
       *[other] { $receiver }의 오류 경로를 처리하거나 코드를 테스트로 옮기십시오.
    }
