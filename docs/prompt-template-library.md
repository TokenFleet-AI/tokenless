# 提示词模板库

用于团队内部统一提任务方式，减少入口误用与角色分工不清的问题。

Owner: baoyx · 版本：v1.0 · 生效日期：2026-05-21

## 1. 小任务模板

```text
使用 coder 处理这个小任务：[任务]
范围限制在：[模块/文件]
只完成当前目标，不做额外扩展。
```

## 2. 常规复杂任务模板

```text
使用 orchestrator 执行这个任务：[任务]
请拆解阶段、分配角色、汇总结果。
主入口 Agent 不要直接包办全部实现。
```

## 3. 多智能体 TDD 模板

```text
使用 orchestrator 执行这个 TDD 任务：[任务]
按 tester → coder → reviewer 顺序推进。
没有 failing tests 不得进入实现，没有 review clear 不得结束。
```

## 4. 强制多智能体模板

```text
使用 swarm-coordinator 接管这个高风险任务：[任务]
请先拆分子任务，再委派执行。
重点关注影响面、测试覆盖、回归风险和最终审查。
```

## 5. 补测试模板

```text
使用 tester 为这个任务补测试：[任务]
目标范围：[模块/文件]
优先覆盖正常路径、边界条件、错误路径。
不要改业务逻辑，除非为了让测试可验证而必须做最小调整。
```

## 导航

- 上一篇：[SPARC 使用规范](./sparc-usage-guideline.md)
- 下一篇：[TDD 规范](./tdd-guideline.md)
- 返回：[Documentation Index](./index.md)

Owner: baoyx · 版本：v1.0 · 生效日期：2026-05-21 · 最后更新：2026-05-21