# lint-violation fixture

故意包含 `unwrap()` 调用，用于验证 clippy `unwrap_used` lint 能在 CI 中失败。
该 crate 不是 workspace 成员，不会破坏主构建。
