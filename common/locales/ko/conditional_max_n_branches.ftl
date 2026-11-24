## 조건식은 얕게 유지해야 합니다.

# `branches` and `limit` are numbers; select on them for plural agreement.
conditional_max_n_branches = “{ $name }”의 분기를 { $limit }개 이하로 줄이십시오.
    .note = “{ $name }”에는 현재 분기가 { $branches }개 있습니다.
    .help = 도우미 함수를 추출하거나 “{ $name }” 부분을 단순화하여 분기를 줄이십시오.
