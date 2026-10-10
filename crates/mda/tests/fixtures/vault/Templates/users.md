---
artifactType: Template
title: Users
language: js
---

```js
const users = [
<VK-each:users:",">
  { id: <VK-users.id>, name: "<VK-users.name>", tags: [<VK-each:users.tags:", ">"<VK-users.tags>"<VK-end:users.tags>] }
<VK-end:users>
];
// all tags: <VK-join:users.tags:" | ">
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
```
