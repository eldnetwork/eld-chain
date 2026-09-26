# Deploy a release to EC2

A `v*.*.*` tag publishes `ghcr.io/eldnetwork/eld-chain:<tag>` through [`.github/workflows/image.yml`](.github/workflows/image.yml). That does not deploy.

Deploy is [`.github/workflows/deploy-ec2.yml`](.github/workflows/deploy-ec2.yml). A developer runs it by hand and must name a tag that already has a published GitHub Release and a successful image workflow. Local compose tags stay in `deploy/.env`. They are usually ahead of GitHub Actions, and this deploy does not read them.

The host is one EC2 machine at `/opt/eld-chain`: four `eld-app` containers and four Tendermint containers. Chain volumes and key files stay across deploys.

## Host prep

On the instance, before the copy. The first three blocks run over SSH as the deploy user (`ubuntu` on the example host).

Install Docker Engine, the Compose v2 plugin, and `rsync`:

```sh
sudo apt-get update
sudo apt-get install -y ca-certificates curl
sudo install -m 0755 -d /etc/apt/keyrings
sudo curl -fsSL https://download.docker.com/linux/ubuntu/gpg -o /etc/apt/keyrings/docker.asc
sudo chmod a+r /etc/apt/keyrings/docker.asc
echo "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/docker.asc] https://download.docker.com/linux/ubuntu $(. /etc/os-release && echo "$VERSION_CODENAME") stable" | sudo tee /etc/apt/sources.list.d/docker.list > /dev/null
sudo apt-get update
sudo apt-get install -y docker-ce docker-ce-cli containerd.io docker-buildx-plugin docker-compose-plugin rsync
```

Add that user to the `docker` group, then disconnect and SSH in again so the group applies:

```sh
sudo usermod -aG docker "$USER"
```

From your machine, this must print a Compose version. Actions uses a non-interactive shell, so `docker` has to be on that shell's `PATH`:

```sh
ssh -i "$EC2_SSH_KEY_FILE" -p "${EC2_SSH_PORT:-22}" "$EC2_USER@$EC2_HOST" docker compose version
```

Create the deploy directory and give it to the deploy user:

```sh
sudo mkdir -p /opt/eld-chain
sudo chown "$USER:$USER" /opt/eld-chain
```

Allow SSH from GitHub-hosted runners. Their address list is thousands of CIDRs, which does not fit in one security group, so open port 22. Run this from a machine with AWS credentials. Chain ports are a separate firewall choice.

```sh
export EC2_SG_ID=sg-0123456789abcdef0
aws ec2 authorize-security-group-ingress \
  --group-id "$EC2_SG_ID" \
  --protocol tcp \
  --port 22 \
  --cidr 0.0.0.0/0
```

## One-time files

Run this once from a checkout of this repo. It `scp`s the remote compose file, the deploy script, public node config, and the local secret files. It does not copy `addrbook.json`.

```sh
export EC2_HOST=203.0.113.10
export EC2_USER=ubuntu
export EC2_SSH_KEY_FILE="$HOME/.ssh/eld-ec2.pem"
export EC2_SSH_PORT=22
# Optional. Writes /opt/eld-chain/.env. Omit to let the first Actions deploy write it.
export ELD_CHAIN_IMAGE=ghcr.io/eldnetwork/eld-chain:v0.0.1
export ELD_TENDERMINT_IMAGE=ghcr.io/eldnetwork/eld-tendermint:v0.34.24-eld.1
./deploy/scripts/docker/remote/cluster/bootstrap-ec2.sh
```

The secret files must already exist in the local cluster tree (`deploy/docker/local/cluster/`). They are gitignored. This script is the only copy of those keys. Later Actions runs exclude them.

Layout on the host:

```text
/opt/eld-chain/
  compose.ghcr.yaml
  deploy-release.sh
  .env
  wallets/
    wallets.json
  nodes/
    1/
      app/
        config.json
        consensus_config.json
        p2p_keypair.json
      tendermint/
        config.toml
        genesis.json
        node_key.json
        priv_validator_key.json
    2/ ...
    3/ ...
    4/ ...
```

Nodes 2–4 use the same filenames as node 1. Named Docker volumes (`deploy_eld-data-*`, `deploy_tendermint-data-*`) are created on the first `up`. They are not part of the scp.

The one-time copy uses the local working tree, which may be ahead of any release. The first Actions deploy replaces public configs with the files from the chosen release tag and leaves the key files in place.

## Publish a tagged release to EC2

1. Merge this deploy path to the default branch. The release you deploy must contain `deploy/docker/remote/cluster/compose.ghcr.yaml` and `deploy/scripts/docker/remote/cluster/deploy-release.sh`.
2. Push a tag. That only builds and publishes the image.

```sh
git tag -a v0.0.42 -m "eld-chain v0.0.42"
git push origin v0.0.42
```

3. Publish a GitHub Release for that tag. A bare tag is not enough. Drafts are rejected. Prereleases are allowed.
4. Wait until the `image` workflow for that tag is green.
5. Run **deploy-ec2** from the Actions tab. Set `app_tag` to that release tag. Leave `tendermint_tag` empty to use the `TENDERMINT_VERSION_TAG_GHCR` variable, or set a tag explicitly.

The job checks out that tag, copies compose and public node configs, rewrites `/opt/eld-chain/.env`, pulls the images, and recreates containers. Volumes and key files stay.

## Env vars

### EC2 `/opt/eld-chain/.env`

Compose reads this file. The deploy workflow rewrites it on every run. Do not put the GHCR token here.

| Name | Example | Role |
|---|---|---|
| `ELD_DEPLOY_ROOT` | `/opt/eld-chain` | Host path for node config and `wallets.json` |
| `ELD_CHAIN_IMAGE` | `ghcr.io/eldnetwork/eld-chain:v0.0.42` | App image, including tag |
| `ELD_TENDERMINT_IMAGE` | `ghcr.io/eldnetwork/eld-tendermint:v0.34.24-eld.1` | Tendermint image, including tag |
| `NODE_APP_VERSION_TAG_GHCR` | `v0.0.42` | Recorded app tag. Compose does not interpolate it |
| `TENDERMINT_VERSION_TAG_GHCR` | `v0.34.24-eld.1` | Recorded Tendermint tag. Compose does not interpolate it |

`GHCR_PULL_TOKEN` and `GHCR_USERNAME` are passed only for `docker login` during a deploy. They are not written to `.env`.

### One-time scp (developer machine)

| Name | Example | Role |
|---|---|---|
| `EC2_HOST` | `203.0.113.10` | Instance address |
| `EC2_USER` | `ubuntu` | SSH user |
| `EC2_SSH_KEY_FILE` | `~/.ssh/eld-ec2.pem` | Path to the private key |
| `EC2_SSH_PORT` | `22` | SSH port. Defaults to 22 |
| `ELD_DEPLOY_ROOT` | `/opt/eld-chain` | Remote directory. Defaults to `/opt/eld-chain` |
| `ELD_CHAIN_IMAGE` | `ghcr.io/eldnetwork/eld-chain:v0.0.1` | Optional. Writes `.env` when set with the Tendermint image |
| `ELD_TENDERMINT_IMAGE` | `ghcr.io/eldnetwork/eld-tendermint:v0.34.24-eld.1` | Optional. Pair with `ELD_CHAIN_IMAGE` |

### GitHub Environment `testnet`

Create the environment and put these on it. The workflow will not run until the environment exists.

| Name | Kind | Example | Role |
|---|---|---|---|
| `EC2_HOST` | secret | `203.0.113.10` | Instance address |
| `EC2_USER` | secret | `ubuntu` | SSH user |
| `EC2_SSH_KEY` | secret | PEM contents | Private key text, not a path |
| `EC2_SSH_PORT` | secret | `22` | SSH port |
| `GHCR_PULL_TOKEN` | secret | GitHub PAT | `read:packages` for `eld-chain` and `eld-tendermint` |
| `GHCR_USERNAME` | secret | GitHub user | Owner of that PAT |
| `TENDERMINT_VERSION_TAG_GHCR` | variable | `v0.34.24-eld.1` | Tendermint tag when the workflow input is empty |

There is no GitHub variable for the app image. The developer types the published release tag into `app_tag` each time.
