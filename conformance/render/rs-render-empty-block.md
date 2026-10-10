---
artifactType: Template
title: rs-render-empty-block
language: js
---

```js
a
<VK-each:none>
x
<VK-end:none>
b
```

vars:
```vks
VK-env:
  - dev
  - prod
VK-users:
  - id: 1
    name: Alice Smith
    tags:
      - core
      - ops
  - id: 2
    name: Bob Jones
    tags: []
VK-db:
  host: localhost
  port: "5432"
VK-none: []
```
