# R3：`evaluator.rs` 模块化（Temporal/Duration 优先）

更新时间：2026-02-12  
任务类型：Phase 1a  
任务状态：In Progress

## 1. 目标

- 拆分 `/Volumes/WorkDrive/Code/github.com/LuQing-Studio/rust/nervusdb/nervusdb-v2-query/src/evaluator.rs`。
- 优先抽离 temporal/duration 函数族，降低表达式求值复杂度。
- 保持错误分类与表达式行为等价。

## 2. 边界

- 允许：求值器内部函数迁移、模块封装、重复代码消除。
- 禁止：改动 public API、错误分类、内建函数外部可见行为。
- 禁止：把语义 bug 修复混入本次结构拆分。

## 3. 文件清单

### 3.1 必改文件

- `/Volumes/WorkDrive/Code/github.com/LuQing-Studio/rust/nervusdb/nervusdb-v2-query/src/evaluator.rs`

### 3.2 新增文件（建议结构）

- `/Volumes/WorkDrive/Code/github.com/LuQing-Studio/rust/nervusdb/nervusdb-v2-query/src/evaluator/evaluator_duration.rs`
- `/Volumes/WorkDrive/Code/github.com/LuQing-Studio/rust/nervusdb/nervusdb-v2-query/src/evaluator/evaluator_timezone.rs`
- `/Volumes/WorkDrive/Code/github.com/LuQing-Studio/rust/nervusdb/nervusdb-v2-query/src/evaluator/evaluator_large_temporal.rs`
- `/Volumes/WorkDrive/Code/github.com/LuQing-Studio/rust/nervusdb/nervusdb-v2-query/src/evaluator/evaluator_temporal_parse.rs`
- `/Volumes/WorkDrive/Code/github.com/LuQing-Studio/rust/nervusdb/nervusdb-v2-query/src/evaluator/evaluator_temporal_math.rs`
- `/Volumes/WorkDrive/Code/github.com/LuQing-Studio/rust/nervusdb/nervusdb-v2-query/src/evaluator/evaluator_temporal_map.rs`
- `/Volumes/WorkDrive/Code/github.com/LuQing-Studio/rust/nervusdb/nervusdb-v2-query/src/evaluator/evaluator_temporal_format.rs`
- `/Volumes/WorkDrive/Code/github.com/LuQing-Studio/rust/nervusdb/nervusdb-v2-query/src/evaluator/evaluator_numeric.rs`
- `/Volumes/WorkDrive/Code/github.com/LuQing-Studio/rust/nervusdb/nervusdb-v2-query/src/evaluator/mod.rs`
- `/Volumes/WorkDrive/Code/github.com/LuQing-Studio/rust/nervusdb/nervusdb-v2-query/src/evaluator/temporal.rs`
- `/Volumes/WorkDrive/Code/github.com/LuQing-Studio/rust/nervusdb/nervusdb-v2-query/src/evaluator/duration.rs`
- `/Volumes/WorkDrive/Code/github.com/LuQing-Studio/rust/nervusdb/nervusdb-v2-query/src/evaluator/functions.rs`
- `/Volumes/WorkDrive/Code/github.com/LuQing-Studio/rust/nervusdb/nervusdb-v2-query/src/evaluator/coercion.rs`

## 4. TDD 拆分步骤

1. 新增失败测试：temporal/duration 边界输入与异常路径。
2. 抽离 temporal 子模块并保持原入口。
3. 抽离 duration 子模块并保持原入口。
4. 抽离函数调度逻辑，保留调用约定。
5. 全门禁回归。

## 4.1 当前进展（2026-02-12）

1. 已完成切片-1（Duration 格式化与构造函数抽离）
   - 从 `evaluator.rs` 抽离 `duration_value / duration_value_wide / duration_iso_components / duration_iso_from_nanos_i128` 到：
     `/Volumes/WorkDrive/Code/github.com/LuQing-Studio/rust/nervusdb/nervusdb-v2-query/src/evaluator/evaluator_duration.rs`
   - `evaluator.rs` 保留原调用入口，改为通过模块导入复用实现，行为保持等价。
   - 当前 `evaluator.rs` 行数从 `4832` 降到 `4709`。
2. 已完成切片-2（Duration 解析函数簇抽离）
   - 从 `evaluator.rs` 抽离 `duration_from_value / duration_from_map / parse_duration_literal / parse_duration_number / parse_duration_seconds_to_nanos` 到：
     `/Volumes/WorkDrive/Code/github.com/LuQing-Studio/rust/nervusdb/nervusdb-v2-query/src/evaluator/evaluator_duration.rs`
   - 新模块内部新增 `duration_map_i64 / duration_map_number / duration_map_number_any`，避免与 `evaluator.rs` 其余 temporal map helper 产生耦合。
   - `evaluator.rs` 继续保留原入口调用（通过 `use evaluator_duration::{...}` 导入），行为保持等价。
   - 当前 `evaluator.rs` 行数从 `4709` 进一步降到 `4444`。
3. 已完成切片-3（Duration 算术函数簇抽离）
   - 从 `evaluator.rs` 抽离 `add_duration_parts / sub_duration_parts / scale_duration_parts` 到：
     `/Volumes/WorkDrive/Code/github.com/LuQing-Studio/rust/nervusdb/nervusdb-v2-query/src/evaluator/evaluator_duration.rs`
   - `multiply_values / divide_values` 保持原调用路径，仅替换为模块导入函数调用。
   - 当前 `evaluator.rs` 行数从 `4444` 进一步降到 `4400`。
4. 已完成切片-4（时区与偏移解析簇抽离）
   - 从 `evaluator.rs` 抽离 `timezone_named_offset / timezone_named_offset_local / timezone_named_offset_standard / parse_fixed_offset / format_offset` 及 DST 规则 helper 到：
     `/Volumes/WorkDrive/Code/github.com/LuQing-Studio/rust/nervusdb/nervusdb-v2-query/src/evaluator/evaluator_timezone.rs`
   - `evaluator.rs` 中 temporal 构造与解析调用点保持不变，仅改为 `use evaluator_timezone::{...}` 导入调用。
   - 当前 `evaluator.rs` 行数从 `4400` 进一步降到 `4136`。
5. 已完成切片-5（LargeTemporal 解析与运算簇抽离）
   - 从 `evaluator.rs` 抽离 `parse_large_date_literal / parse_large_localdatetime_literal / format_large_date_literal / format_large_localdatetime_literal / large_months_and_days_between / large_localdatetime_epoch_nanos` 到：
     `/Volumes/WorkDrive/Code/github.com/LuQing-Studio/rust/nervusdb/nervusdb-v2-query/src/evaluator/evaluator_large_temporal.rs`
   - `evaluate_large_duration_between / parse_large_temporal_arg` 调用点保持不变，仍通过原入口使用这些实现。
   - 当前 `evaluator.rs` 行数从 `4136` 进一步降到 `3932`。
6. 已完成切片-6（Temporal 字符串解析簇抽离）
   - 从 `evaluator.rs` 抽离 `extract_timezone_name / parse_temporal_string / find_offset_split_index / parse_time_literal / parse_date_literal / parse_week_date_components / parse_ordinal_date_components / parse_year_month_components` 到：
     `/Volumes/WorkDrive/Code/github.com/LuQing-Studio/rust/nervusdb/nervusdb-v2-query/src/evaluator/evaluator_temporal_parse.rs`
   - `evaluator.rs` 保留原入口调用，只通过 `use evaluator_temporal_parse::{...}` 导入，不改变调用路径和错误分支。
   - 当前 `evaluator.rs` 行数从 `3932` 进一步降到 `3662`。
7. 已完成切片-7（Temporal 基础运算函数簇抽离）
   - 从 `evaluator.rs` 抽离 `compare_time_with_offset / compare_time_of_day / time_of_day_nanos / shift_time_of_day / add_months` 到：
     `/Volumes/WorkDrive/Code/github.com/LuQing-Studio/rust/nervusdb/nervusdb-v2-query/src/evaluator/evaluator_temporal_math.rs`
   - `evaluator.rs` 调用点保持不变，仅替换为 `use evaluator_temporal_math::{...}` 导入。
   - 当前 `evaluator.rs` 行数从 `3662` 进一步降到 `3617`。
8. 已完成切片-8（Temporal map 构造簇抽离）
   - 从 `evaluator.rs` 抽离 `make_date_from_map / make_time_from_map / weekday_from_cypher / cypher_day_of_week / map_i64 / map_i32 / map_u32 / map_string` 到：
     `/Volumes/WorkDrive/Code/github.com/LuQing-Studio/rust/nervusdb/nervusdb-v2-query/src/evaluator/evaluator_temporal_map.rs`
   - `evaluator.rs` 调用点保持不变，改为 `use evaluator_temporal_map::{...}` 导入。
   - 当前 `evaluator.rs` 行数从 `3617` 进一步降到 `3473`。
9. 已完成切片-9（Temporal 格式化函数簇抽离）
   - 从 `evaluator.rs` 抽离 `format_time_literal / format_datetime_literal / format_datetime_with_offset_literal` 到：
     `/Volumes/WorkDrive/Code/github.com/LuQing-Studio/rust/nervusdb/nervusdb-v2-query/src/evaluator/evaluator_temporal_format.rs`
   - `evaluator.rs` 调用点保持不变，改为 `use evaluator_temporal_format::{...}` 导入。
   - 当前 `evaluator.rs` 行数从 `3473` 进一步降到 `3437`。
10. 已完成切片-10（Numeric helper 簇抽离）
   - 从 `evaluator.rs` 抽离 `value_as_f64 / value_as_i64 / numeric_binop / numeric_div / numeric_mod / numeric_pow` 到：
     `/Volumes/WorkDrive/Code/github.com/LuQing-Studio/rust/nervusdb/nervusdb-v2-query/src/evaluator/evaluator_numeric.rs`
   - `value_as_i64` 的浮点输入约束（`fract==0` 且在 `i64` 范围）与原行为保持一致；`numeric_pow` 的 `Int^Int` 分支保持原先返回 `Float`。
   - 当前 `evaluator.rs` 行数从 `3437` 进一步降到 `3372`。
11. 已完成切片-11（Comparison/Ordering helper 簇抽离）
   - 从 `evaluator.rs` 抽离 `compare_values / compare_numbers_for_range / compare_lists_for_range / compare_value_for_list_ordering / compare_lists_ordering / order_compare_non_null / compare_strings_with_temporal` 到：
     `/Volumes/WorkDrive/Code/github.com/LuQing-Studio/rust/nervusdb/nervusdb-v2-query/src/evaluator/evaluator_compare.rs`
   - `BinaryOperator::{<,<=,>,>=}` 与 `order_compare` 调用点保持不变，仅改为 `use evaluator_compare::{...}` 导入。
   - 当前 `evaluator.rs` 行数从 `3372` 进一步降到 `3261`。
12. 已完成切片-12（String/Membership helper 簇抽离）
   - 从 `evaluator.rs` 抽离 `string_predicate / in_list` 到：
     `/Volumes/WorkDrive/Code/github.com/LuQing-Studio/rust/nervusdb/nervusdb-v2-query/src/evaluator/evaluator_membership.rs`
   - 原二元表达式调用点保持不变，`IN` 语义继续通过 `cypher_equals` 判定，未改行为分支。
   - 当前 `evaluator.rs` 行数从 `3261` 进一步降到 `3229`。
13. 已完成切片-13（Arithmetic helper 簇抽离）
   - 从 `evaluator.rs` 抽离 `add_values / subtract_values / multiply_values / divide_values` 到：
     `/Volumes/WorkDrive/Code/github.com/LuQing-Studio/rust/nervusdb/nervusdb-v2-query/src/evaluator/evaluator_arithmetic.rs`
   - temporal + duration + numeric 的分支顺序保持不变，调用入口仅改为模块导入。
   - 当前 `evaluator.rs` 行数从 `3229` 进一步降到 `3127`。
14. 已完成切片-14（Cypher equality helper 簇抽离）
   - 从 `evaluator.rs` 抽离 `cypher_equals / float_equals_int / cypher_equals_sequence / cypher_equals_map` 到：
     `/Volumes/WorkDrive/Code/github.com/LuQing-Studio/rust/nervusdb/nervusdb-v2-query/src/evaluator/evaluator_equality.rs`
   - `BinaryOperator::{=,<>}` 与 `IN` 语义调用点保持不变，统一复用 `evaluator_equality::cypher_equals`。
   - 当前 `evaluator.rs` 行数从 `3127` 进一步降到 `3053`。
15. 回归结果
   - `cargo test -p nervusdb-v2-query --lib`（切片-1后）通过。
   - `cargo test -p nervusdb-v2 --test t311_expressions`（切片-1后）通过。
   - `cargo test -p nervusdb-v2 --test t313_functions`（切片-1后）通过。
   - `cargo test -p nervusdb-v2 --test t333_varlen_direction`（切片-1后）通过。
   - `cargo test -p nervusdb-v2-query --lib`（切片-2后）通过。
   - `cargo test -p nervusdb-v2 --test t311_expressions`（切片-2后）通过。
   - `cargo test -p nervusdb-v2 --test t313_functions`（切片-2后）通过。
   - `cargo test -p nervusdb-v2 --test t333_varlen_direction`（切片-2后）通过。
   - `cargo test -p nervusdb-v2-query --lib`（切片-3后）通过。
   - `cargo test -p nervusdb-v2 --test t311_expressions`（切片-3后）通过。
   - `cargo test -p nervusdb-v2 --test t313_functions`（切片-3后）通过。
   - `cargo test -p nervusdb-v2 --test t333_varlen_direction`（切片-3后）通过。
   - `cargo test -p nervusdb-v2-query --lib`（切片-4后）通过。
   - `cargo test -p nervusdb-v2 --test t311_expressions`（切片-4后）通过。
   - `cargo test -p nervusdb-v2 --test t313_functions`（切片-4后）通过。
   - `cargo test -p nervusdb-v2 --test t333_varlen_direction`（切片-4后）通过。
   - `bash scripts/workspace_quick_test.sh`（切片-4后阶段门禁）通过。
   - `bash scripts/contract_smoke.sh`（切片-4后阶段门禁）通过。
   - `bash scripts/binding_smoke.sh`（切片-4后阶段门禁）通过（含既有 `pyo3 gil-refs` warning）。
   - `cargo test -p nervusdb-v2-query --lib`（切片-5后）通过。
   - `cargo test -p nervusdb-v2 --test t311_expressions`（切片-5后）通过。
   - `cargo test -p nervusdb-v2 --test t313_functions`（切片-5后）通过。
   - `cargo test -p nervusdb-v2 --test t333_varlen_direction`（切片-5后）通过。
   - `cargo test -p nervusdb-v2-query --lib`（切片-6后）通过。
   - `cargo test -p nervusdb-v2 --test t311_expressions`（切片-6后）通过。
   - `cargo test -p nervusdb-v2 --test t313_functions`（切片-6后）通过。
   - `cargo test -p nervusdb-v2 --test t333_varlen_direction`（切片-6后）通过。
   - `cargo test -p nervusdb-v2-query --lib`（切片-7后）通过。
   - `cargo test -p nervusdb-v2 --test t311_expressions`（切片-7后）通过。
   - `cargo test -p nervusdb-v2 --test t313_functions`（切片-7后）通过。
   - `cargo test -p nervusdb-v2 --test t333_varlen_direction`（切片-7后）通过。
   - `bash scripts/workspace_quick_test.sh`（切片-7/8后阶段门禁）通过。
   - `bash scripts/contract_smoke.sh`（切片-7/8后阶段门禁）通过。
   - `bash scripts/binding_smoke.sh`（切片-7/8后阶段门禁）通过（含既有 `pyo3 gil-refs` warning）。
   - `cargo test -p nervusdb-v2-query --lib`（切片-8后）通过。
   - `cargo test -p nervusdb-v2 --test t311_expressions`（切片-8后）通过。
   - `cargo test -p nervusdb-v2 --test t313_functions`（切片-8后）通过。
   - `cargo test -p nervusdb-v2 --test t333_varlen_direction`（切片-8后）通过。
   - `cargo test -p nervusdb-v2-query --lib`（切片-9后）通过。
   - `cargo test -p nervusdb-v2 --test t311_expressions`（切片-9后）通过。
   - `cargo test -p nervusdb-v2 --test t313_functions`（切片-9后）通过。
   - `cargo test -p nervusdb-v2 --test t333_varlen_direction`（切片-9后）通过。
   - `cargo test -p nervusdb-v2-query --lib`（切片-10后）通过。
   - `cargo test -p nervusdb-v2 --test t311_expressions`（切片-10后）通过。
   - `cargo test -p nervusdb-v2 --test t313_functions`（切片-10后）通过。
   - `cargo test -p nervusdb-v2 --test t333_varlen_direction`（切片-10后）通过。
   - `cargo test -p nervusdb-v2-query --lib`（切片-11后）通过。
   - `cargo test -p nervusdb-v2 --test t311_expressions`（切片-11后）通过。
   - `cargo test -p nervusdb-v2 --test t313_functions`（切片-11后）通过。
   - `cargo test -p nervusdb-v2 --test t333_varlen_direction`（切片-11后）通过。
   - `cargo test -p nervusdb-v2-query --lib`（切片-12后）通过。
   - `cargo test -p nervusdb-v2 --test t311_expressions`（切片-12后）通过。
   - `cargo test -p nervusdb-v2 --test t313_functions`（切片-12后）通过。
   - `cargo test -p nervusdb-v2 --test t333_varlen_direction`（切片-12后）通过。
   - `cargo test -p nervusdb-v2-query --lib`（切片-13后）通过。
   - `cargo test -p nervusdb-v2 --test t311_expressions`（切片-13后）通过。
   - `cargo test -p nervusdb-v2 --test t313_functions`（切片-13后）通过。
   - `cargo test -p nervusdb-v2 --test t333_varlen_direction`（切片-13后）通过。
   - `cargo test -p nervusdb-v2-query --lib`（切片-14后）通过。
   - `cargo test -p nervusdb-v2 --test t311_expressions`（切片-14后）通过。
   - `cargo test -p nervusdb-v2 --test t313_functions`（切片-14后）通过。
   - `cargo test -p nervusdb-v2 --test t333_varlen_direction`（切片-14后）通过。
   - `bash scripts/workspace_quick_test.sh`（切片-12/13/14后阶段门禁）通过。
   - `bash scripts/contract_smoke.sh`（切片-12/13/14后阶段门禁）通过。
   - `bash scripts/binding_smoke.sh`（切片-12/13/14后阶段门禁）通过（含既有 `pyo3 gil-refs` warning）。

## 5. 测试清单

- `/Volumes/WorkDrive/Code/github.com/LuQing-Studio/rust/nervusdb/nervusdb-v2/tests/t311_expressions.rs`
- `/Volumes/WorkDrive/Code/github.com/LuQing-Studio/rust/nervusdb/nervusdb-v2/tests/t312_unary.rs`
- `/Volumes/WorkDrive/Code/github.com/LuQing-Studio/rust/nervusdb/nervusdb-v2/tests/t313_functions.rs`
- `/Volumes/WorkDrive/Code/github.com/LuQing-Studio/rust/nervusdb/nervusdb-v2/tests/t332_binding_validation.rs`

## 6. 风险与回滚

- 风险：类型转换与 temporal 构造顺序改变导致错误分支偏移。
- 检测：对照错误分类 `Syntax/Execution` 与 message 前缀。
- 回滚：失败即回滚该 PR，不串联修复。

## 7. 完成定义（DoD）

- evaluator 单文件复杂度显著下降。
- temporal/duration 行为与基线一致。
- 全门禁通过。
