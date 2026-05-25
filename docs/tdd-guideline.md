# TDD 规范

Owner: baoyx · 版本：v1.0 · 生效日期：2026-05-21

团队统一约定：复杂任务禁止直接以 `tdd` 作为入口；正确方式是 `orchestrator/swarm-coordinator + TDD mandatory`。

## 1. 推荐顺序

- `tester`：测试计划与 failing tests
- `coder`：基于测试实现
- `tester`：回归验证
- `reviewer`：最终审查

## 2. 最小门禁

- `TEST_PLAN_READY`
- `RED_CONFIRMED`
- `IMPLEMENTATION_DONE`
- `REVIEW_CLEAR`

## 3. 禁止事项

- 没有 failing tests 直接进入实现
- 主入口 Agent 跳过测试阶段自行编码
- 没有 review clear 就宣告完成

## 4. 推荐提示词

- 使用 `orchestrator` 执行 [任务]，按 `tester → coder → reviewer` 顺序推进。
- 没有 failing tests 不得进入实现，没有 review clear 不得结束。

## 导航

- 上一篇：[提示词模板库](./prompt-template-library.md)
- 下一篇：[高风险任务处理规范](./high-risk-task-guideline.md)
- 返回：[SPARC 使用规范](./sparc-usage-guideline.md)

Owner: baoyx · 版本：v1.0 · 生效日期：2026-05-21 · 最后更新：2026-05-21