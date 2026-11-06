---
artifactType: LeetCode
leetcodeType: function
title: Two Sum
difficulty: easy
status: unsolved
algorithm: hash-map
tags: [leetcode, arrays]
---

Given an array, return indices.

```yaml leetcode
function: twoSum
params:
  - name: nums
    type: int[]
  - name: target
    type: int
returns: int[]
```

## Examples
```example
input: nums = [2,7], target = 9
output: [0,1]
```

```yaml leetcode
test:
  type: call
  timeoutMs: 3000
practice:
  timeLimit: 20
  locked: true
  options: [noSnippets]
```

## Tests
```json
[
  { "input": { "nums": [2,7], "target": 9 }, "expected": [0,1] }
]
```

## Final Tests
```json
[
  { "input": { "nums": [1,5,3], "target": 8 }, "expected": [1,2] }
]
```

# Setup

## Python
```python
def twoSum(nums, target):
    pass
```

## JavaScript
```javascript
function twoSum(nums, target) {}
```

# Solutions

## Python
### Hash Map
<!-- meta: { "solved_at": "2026-07-15T14:32:00Z", "duration": "8m22s" } -->
```python
def twoSum(nums, target):
    seen = {}
```

### Brute
```python
def twoSum(nums, target):
    return []
```

# Attempts

## Python
<!-- attempt: { "at": "2026-07-16T10:00:00Z", "duration": "3m", "passed": true, "bigO": "O(n)", "confidence": "high" } -->
```python
def twoSum(n, t):
    return [0, 1]
```

<!-- attempt: { "at": "2026-07-15T10:00:00Z", "duration": "9m" } -->
```python
broken = True
```
