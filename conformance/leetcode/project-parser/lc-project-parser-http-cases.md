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
  checks:
    - name: sum endpoint
      kind: http
      package: api
    - name: page
      kind: dom-assert
      file: index.html
```

## Tests
```json check="sum endpoint"
[
  { "request": { "method": "GET", "path": "/sum?a=2&b=3" }, "expect": { "status": 200, "body": { "sum": 5 } } },
  { "expect": { "status": 404 }, "request": { "method": "GET", "path": "/nope" }, "note": "key order kept" }
]
```

```json check=page
[{"input": {}, "expected": 1}]
```

## Final Tests
```json check="sum endpoint"
[{ "request": { "method": "GET", "path": "/sum?a=-5&b=-5" }, "expect": { "status": 200, "body": { "sum": -10 } } }]
```
