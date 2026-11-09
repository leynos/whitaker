//! Macro-free fixture pinning cognitive complexity scores for parity checks.
// baseline: no authored branching scores 1.
pub fn baseline() {}
// one_if: one authored `if` adds one to the baseline.
pub fn one_if(flag: bool) { if flag {} }
// two_ifs: two authored `if` expressions add two.
pub fn two_ifs(flag: bool) { if flag {} if !flag {} }
// multi_arm_match: a three-arm match is one branch.
pub fn multi_arm_match(value: u8) { match value { 0 => {}, 1 => {}, _ => {} } }
// single_arm_match: a single-arm match is not a branch.
pub fn single_arm_match(value: u8) { match value { _ => {} } }
// guarded_match: a two-arm match plus an authored guard is two branches.
pub fn guarded_match(value: Option<u8>) { match value { Some(v) if v > 0 => {}, _ => {} } }
// adjacent_returns: one `if` and two returns separated by their value expressions.
pub fn adjacent_returns(flag: bool) -> u8 { if flag { return 1; } return 2; }
// separated_returns: two `if` branches and two returns in a non-Result body.
pub fn separated_returns(flag: bool, other: bool) -> u8 { if flag { return 1; } if other { return 2; } 3 }
// separated_result_returns: the same shape, but a Result body credits returns in full.
pub fn separated_result_returns(flag: bool, other: bool) -> Result<u8, ()> { if flag { return Ok(1); } if other { return Ok(2); } Ok(3) }
// returns_result: one `if` and one credited return in a Result body.
pub fn returns_result(flag: bool) -> Result<u8, ()> { if flag { return Ok(1); } Ok(2) }
// source_for: `for` desugars to a two-arm match.
pub fn source_for(values: &[u8]) { for value in values { let _ = *value; } }
// source_while: `while` desugars to an `if`.
pub fn source_while(mut count: u8) { while count > 0 { count -= 1; } }
// source_for_if: the desugared `for` match plus an authored `if`.
pub fn source_for_if(values: &[u8]) { for value in values { if *value > 0 {} } }
// try_result: `?` lowers to a two-arm match and one return in a Result body.
pub fn try_result(input: Result<u8, ()>) -> Result<u8, ()> { let value = input?; Ok(value) }
// try_option: `?` on Option halves the credit for its single return.
pub fn try_option(input: Option<u8>) -> Option<u8> { let value = input?; Some(value) }
// closure_body: the closure scores 2 and the enclosing function scores 1.
pub fn closure_body() { let callback = |flag: bool| if flag {}; callback(true); }
// async_if: the coroutine wrapper is not scored; the authored `if` is.
pub async fn async_if(flag: bool) { if flag {} }
// async_await: `await` lowers to a two-arm match that both tools charge.
pub async fn async_await(future: impl core::future::Future<Output = u8>) -> u8 { future.await }
