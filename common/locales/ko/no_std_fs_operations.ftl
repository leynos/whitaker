## 케이퍼빌리티 기반 I/O를 적용하기 위해 std::fs 작업을 제한합니다.

no_std_fs_operations = std::fs 작업 `{ $operation }`은(는) 케이퍼빌리티 기반 파일 시스템 정책을 우회합니다.
    .note = std::fs는 암묵적인 작업 디렉터리에 접근하므로, 호출자가 케이퍼빌리티 범위를 선택할 수 있도록 대신 `cap_std::fs::Dir` 핸들과 camino 경로를 받으십시오.
    .help = std::fs를 직접 호출하는 대신 `cap_std::fs::Dir`과 `camino::Utf8Path`/`Utf8PathBuf` 매개변수를 API를 통해 전달하십시오.
