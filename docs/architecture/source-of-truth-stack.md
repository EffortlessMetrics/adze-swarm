# Source-of-truth stack

Adze uses a linked artifact chain so humans and agents can answer what/why/how
without relying on chat history.

```text
Roadmap -> Proposal -> Spec -> ADR -> Lane tracker -> Implementation plan
-> PR -> Proof -> Support/policy -> Closeout
```

Durable artifacts for this chain live in `.adze-spec/`.

External agent/tool namespaces (for example `.codex/` and `.spec/`) are not the
owned durable control plane for this stack.
