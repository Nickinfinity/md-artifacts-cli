---
artifactType: Template
title: rs-render-join-default
language: js
---

```js
<VK-join:env>
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
