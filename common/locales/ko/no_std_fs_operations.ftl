## 케이퍼빌리티 기반 I/O를 적용하기 위해 std::fs 작업을 제한합니다.

no_std_fs_operations = std::fs 작업 `{ $operation }`은(는) 케이퍼빌리티 기반 파일 시스템 정책을 우회합니다.
    .note = std::fs는 암묵적인 작업 디렉터리에 접근하므로, 호출자가 케이퍼빌리티 범위를 선택할 수 있도록 대신 `cap_std::fs::Dir` 핸들과 camino 경로를 받으십시오.
    .help = std::fs를 직접 호출하는 대신 `cap_std::fs::Dir`과 `camino::Utf8Path`/`Utf8PathBuf` 매개변수를 API를 통해 전달하십시오.

no_std_fs_operations_unknown_keys = no_std_fs_operations { $version }(리비전 { $revision })의 [no_std_fs_operations]에 있는 알 수 없는 키 { $keys }을(를) 무시합니다.
    .note = 이 섹션의 다른 키는 계속 적용됩니다. 인식되지 않는 키는 아무런 효과가 없습니다.
    .help = 이 키를 이해하는 리비전으로 Whitaker 제품군을 업데이트하거나 dylint.toml에서 제거하십시오.

no_std_fs_operations_discarded_section = no_std_fs_operations { $version }(리비전 { $revision })의 [no_std_fs_operations] 구성을 버립니다: { $error }
    .note = 어떤 제외도 적용되지 않습니다. lint는 기본 설정을 사용합니다.
    .help = dylint.toml의 잘못된 값을 수정하거나 키를 이해하도록 Whitaker 제품군을 업데이트하십시오.
