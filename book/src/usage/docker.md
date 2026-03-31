# Docker Containers

previewf can browse and preview files inside running Docker containers. There are two modes:

- **Host-to-container** (`docker` subcommand + web UI): preview container files from your host machine via `previewf docker serve`
- **Container-native** (`--host 0.0.0.0`): run previewf inside a container and expose it via port mapping

## Command: docker ls

List running Docker containers:

```
previewf docker ls

Arguments:
  (none)
```

Example output:

```
CONTAINER           IMAGE               STATUS
my-app              node:20-alpine      Up 3 hours
api-server          rust:1.76           Up 47 minutes
db-migrate          postgres:16         Up 2 minutes
```

Only running containers are shown. Stopped containers are omitted.

## Command: docker serve

Serve files from inside a running container:

```
previewf docker serve <CONTAINER> <PATH> [OPTIONS]

Arguments:
  <CONTAINER>    Container name or ID
  <PATH>         Absolute path inside the container

Options:
  -p, --port <PORT>                   Port to listen on [default: 3000]
      --poll-interval <MILLISECONDS>  File change poll interval [default: 1000]
  -h, --help                          Print help
```

Examples:

```bash
# Serve /app/docs inside container "my-app"
previewf docker serve my-app /app/docs

# Custom port and faster poll interval
previewf docker serve my-app /app/docs --port 8080 --poll-interval 500

# Serve a single file
previewf docker serve my-app /app/README.md
```

Visiting `http://localhost:3000` shows the same directory listing or file viewer as `serve`, but all file reads run through `docker exec`.

## Browser-Based Discovery

When you run `previewf serve` on a host that has Docker available, the index page includes a **Docker Containers** section below the local file listing:

```
+------------------------------------------+
|  previewf   ~/docs/            [theme]   |
+------------------------------------------+
|                                          |
|   * architecture.md        3 flags       |
|   * plan.md                1 flag        |
|                                          |
|   Docker Containers                      |
|   > my-app                               |
|   > api-server                           |
|                                          |
+------------------------------------------+
```

Clicking a container name navigates to `/docker/<container>/` which lists the container's root filesystem. From there you can browse directories and click markdown files to open them in the viewer.

If Docker is not installed or not running, the section is omitted silently.

## Routes

Docker-specific routes mirror the local routes under a `/docker/:container/` prefix:

| Route | Method | Purpose |
|-------|--------|---------|
| `/api/docker/containers` | GET | Return running containers as JSON |
| `/docker/:container/` | GET | Directory listing of container root |
| `/docker/:container/view/*filepath` | GET | Render a markdown file from container |
| `/docker/:container/raw/*filepath` | GET | Serve an HTML file from container |
| `/docker/:container/flags/*filepath` | GET | Return flags in a container file as JSON |
| `/docker/:container/flag/*filepath` | POST | Create a new flag in a container file |
| `/docker/:container/ws` | GET | WebSocket endpoint for container live reload |

All file reads go through `DockerSource`, which runs `docker exec <container> cat <path>` or `docker exec <container> ls <path>` as needed.

## Live Reload

Container file watching uses polling rather than platform-native events (inotify/FSEvents cannot observe container filesystem changes from the host).

The `DockerPollWatcher` runs on a configurable interval:

1. On each tick, it runs `docker exec <container> stat <path>` to check the last-modified time
2. If the modification time changed, it broadcasts a reload notification
3. The WebSocket client receives the notification and calls `location.reload()`

Polling only runs while a file is actively being viewed. When no browser has the WebSocket connected, polling suspends.

The default interval is 1000ms. For faster feedback during active editing:

```bash
previewf docker serve my-app /app/docs --poll-interval 200
```

## Security

**Container name validation.** Container names and IDs are validated against `[a-zA-Z0-9_.-]+` before use. Any name containing shell metacharacters is rejected with a 400 error.

**Path normalization.** Request paths are normalized to remove `..` components before being passed to `docker exec`. A request for `/docker/my-app/view/../../etc/passwd` is rejected with a 400 error.

**No shell invocation.** All `docker exec` calls use `std::process::Command` with separate arguments, never a shell string. There is no interpolation that could be exploited by a crafted container name or path.

## Troubleshooting

**Docker not found**

```
Error: docker executable not found in PATH
```

Install Docker Desktop or the Docker CLI and ensure `docker` is on your `PATH`.

**Container not running**

```
Error: container "my-app" is not running
```

Start the container with `docker start my-app` and retry.

**Path not found inside container**

```
Error: path "/app/docs" not found in container "my-app"
```

Verify the path exists inside the container:

```bash
docker exec my-app ls /app/docs
```

**Permission denied**

```
Error: permission denied reading "/root/.ssh" in container "my-app"
```

The `docker exec` command runs as the container's default user. If the path requires root, either start the container with a different user or run `docker exec` as root manually to verify access.
