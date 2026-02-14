# 🚀 Cloudflare Tunnel: Quick-Add Cheat Sheet

**Tunnel Name:** `GCP_DNS_NAME`

**Config Path:** `/etc/cloudflared/config.yml`

### Step 1: Register DNS Record

```bash
cloudflared tunnel route dns GCP_DNS_NAME <new.domain.com>

```

### Step 2: Edit Configuration

```bash
vim /etc/cloudflared/config.yml

```

**Add to `ingress` section (above 404 rule):**

```yaml
  - hostname: <new.domain.com>
    service: http://localhost:<port>

```

### Step 3: Validate YAML Syntax

```bash
cloudflared tunnel ingress validate

```

### Step 4: Restart Service

```bash
systemctl restart cloudflared

```

### Step 5: Monitor Live Logs

```bash
tail -f /var/log/cloudflared.log

```
