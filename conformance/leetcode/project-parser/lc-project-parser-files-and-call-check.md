---
artifactType: LeetCode
leetcodeType: package
title: Case
difficulty: easy
status: unsolved
tags: [leetcode]
---

```yaml leetcode
test:
  timeoutMs: 10000
  checks:
    - name: cat filter
      kind: call
      file: src/lib/cat.ts
      function: f
```

## Files

```typescript path=src/lib/cat.ts role=editable
export function f() {}
```

```css path=src/app.css role=readonly
body {}
```

```json path=package.json role=hidden
{}
```

## Tests
```json check="cat filter"
[{"input": {"n": 1}, "expected": [1]}]
```

# Solutions

```typescript path=src/lib/cat.ts
export function f() { return [1]; }
```
