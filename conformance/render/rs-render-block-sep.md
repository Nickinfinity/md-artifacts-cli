---
artifactType: Template
title: rs-render-block-sep
language: js
---

```js
items:
<VK-each:env:",">
- <VK-env>
<VK-end:env>
done
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
