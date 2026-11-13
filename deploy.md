# Deploy the cluster to EC2

An `eld-app-v*.*.*` tag publishes `ghcr.io/eldnetwork/eld-chain:<tag>` through [`.github/workflows/eld-app-image.yml`](.github/workflows/eld-app-image.yml). That does not deploy.

Copy `deploy/docker/remote/cluster` to `~/cluster` on the host. SSH settings and image tags come from `deploy/.env`.

```sh
./deploy/scripts/docker/remote/cluster/copy-cluster-to-ec2.sh
./deploy/scripts/docker/remote/cluster/connect-ec2.sh
```

Start and stop use `deploy/docker/remote/cluster/compose.ghcr.tm-rs.yaml` and `deploy/.env`:

```sh
./deploy/scripts/docker/remote/cluster/cluster-start-without-history.ghcr.tm-rs.sh
./deploy/scripts/docker/remote/cluster/cluster-start-without-history.ghcr.tm-rs.sh --with-faucet --with-analytics
./deploy/scripts/docker/remote/cluster/cluster-stop.ghcr.tm-rs.sh
```
