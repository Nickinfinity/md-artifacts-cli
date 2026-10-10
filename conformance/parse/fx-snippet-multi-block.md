---
artifactType: Snippet
title: API URLs
description: Development and production API base URLs.
tags: [api, urls]
---
<!-- canonical: multi-block snippet, 2 blocks with per-block descriptions and vks fences, 2 tags -->

## Development
Local development server URL.

```javascript
const baseUrl = 'http://localhost:<VK-PORT>';
```

### VKs:

```vks
VK-PORT=3000
```

## Production
Production API base URL.

```javascript
const baseUrl = 'https://api.<VK-DOMAIN>';
```

### VKs:

```vks
VK-DOMAIN=example.com
```
