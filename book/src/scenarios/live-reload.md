# Scenario: Live Reload

This walkthrough traces the live reload mechanism in detail: how file changes are detected, how they propagate through the system, and how the browser updates. Live reload is one of previewf's most important features for a smooth editing workflow.

## The Mechanism at a Glance

```
File saved on disk
    |  (OS-level notification)
    v
notify crate (platform-native watcher)
    |  (callback fires on notify thread)
    v
broadcast::Sender<PathBuf>
    |  (delivers to all subscribers)
    v
WebSocket task (one per connected browser)
    |  (sends "reload" message)
    v
JavaScript WebSocket client
    |  (receives message)
    v
location.reload()
    |  (browser fetches fresh content)
    v
Server re-renders the page
```

## Step 1: Server Startup with File Watching

When the server starts, the file watcher is initialized alongside the HTTP server.

### FileWatcher creation

```rust
let (mut watcher, mut rx) = FileWatcher::new(path.clone())?;
watcher.watch()?;
```

Inside `FileWatcher::new`:

1. A `broadcast::channel(100)` is created. The capacity of 100 means up to 100 unread messages can be buffered per receiver. If a receiver falls behind, older messages are dropped (this is fine -- we only need to know that something changed, not every individual change).

2. The `FileWatcher` struct holds:
   - `path`: The directory or file to watch
   - `watcher`: An `Option<RecommendedWatcher>` (None until `watch()` is called)
   - `sender`: The broadcast sender

### Starting the watch

Inside `FileWatcher::watch`:

```rust
let mut watcher = notify::recommended_watcher(move |res: Result<Event, Error>| {
    if let Ok(event) = res {
        if event.kind.is_modify() || event.kind.is_create() {
            for path in event.paths {
                let _ = sender.send(path);
            }
        }
    }
})?;

let mode = if self.path.is_dir() {
    RecursiveMode::Recursive
} else {
    RecursiveMode::NonRecursive
};

watcher.watch(&self.path, mode)?;
```

**Platform-native watchers:**

| Platform | Backend | How It Works |
|----------|---------|-------------|
| macOS | FSEvents | The OS kernel tracks filesystem events and delivers them via a stream |
| Linux | inotify | Kernel-level file notification system via file descriptors |
| Windows | ReadDirectoryChangesW | Win32 API for directory change notifications |

The `notify::recommended_watcher` function selects the best backend for the current platform automatically.

**Recursive vs non-recursive:** For directories, `RecursiveMode::Recursive` watches all subdirectories. For single files, `RecursiveMode::NonRecursive` watches just that file (and its parent directory, due to how some editors save files).

### The callback

The callback runs on notify's internal thread (not a tokio task). It:

1. Checks if the event is a modify or create event (ignoring delete, rename, etc.)
2. Sends the changed path through the broadcast channel
3. Uses `let _ = sender.send(path)` -- the result is intentionally ignored because a send failure means there are no receivers, which is harmless

## Step 2: WebSocket Connection

When a browser loads a page served by previewf, the JavaScript establishes a WebSocket connection.

### Client side

```javascript
function connectWebSocket() {
    var protocol = location.protocol === 'https:' ? 'wss:' : 'ws:';
    var ws = new WebSocket(protocol + '//' + location.host + '/ws');

    ws.onopen = function() {
        statusConnection.classList.remove('disconnected');
    };

    ws.onmessage = function(event) {
        if (event.data === 'reload') {
            location.reload();
        }
    };

    ws.onclose = function() {
        statusConnection.classList.add('disconnected');
        setTimeout(connectWebSocket, 2000);
    };

    ws.onerror = function() {
        ws.close();
    };
}

connectWebSocket();
```

Key behaviors:
- **Auto-reconnect:** On connection close, wait 2 seconds and retry. This handles server restarts, network interruptions, and temporary disconnections.
- **Status indicator:** The connection state is shown in the status bar (green dot = connected, red = disconnected).
- **Simple protocol:** The only message the client cares about is the string `"reload"`.

### Server side

```rust
async fn ws_handler(
    State(state): State<AppState>,
    ws: WebSocketUpgrade,
) -> Response {
    let rx = state.reload_tx.subscribe();
    ws.on_upgrade(|socket| handle_ws(socket, rx))
}
```

The `ws_handler`:
1. Creates a new broadcast receiver by calling `subscribe()` on the shared sender
2. Upgrades the HTTP connection to WebSocket
3. Passes the socket and receiver to `handle_ws`, which runs as a long-lived tokio task

```rust
async fn handle_ws(mut socket: WebSocket, mut rx: broadcast::Receiver<()>) {
    loop {
        tokio::select! {
            result = rx.recv() => {
                match result {
                    Ok(()) => {
                        if socket.send(Message::Text("reload".into())).await.is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Close(_))) | None => break,
                    _ => {}
                }
            }
        }
    }
}
```

The `handle_ws` function runs a `tokio::select!` loop that waits on two futures simultaneously:

1. **Broadcast receive:** When the file watcher (or flag creation handler) sends a notification, send `"reload"` to the client
2. **Socket receive:** Handle client-sent messages (mainly close messages)

The loop exits when:
- The broadcast channel is dropped (sender gone)
- The WebSocket client disconnects
- Sending to the WebSocket fails (client gone)

## Step 3: A File Changes

Suppose the user edits `plan.md` in VS Code and saves.

### What the OS does

**On macOS (FSEvents):**
1. VS Code writes to a temporary file
2. VS Code renames the temporary file to `plan.md` (atomic save)
3. FSEvents coalesces the events and delivers a notification to the registered callback

**On Linux (inotify):**
1. VS Code opens `plan.md` for writing
2. Writes new content
3. Closes the file
4. inotify delivers `IN_MODIFY` and `IN_CLOSE_WRITE` events

### What notify does

notify's callback fires:

```rust
move |res: Result<Event, Error>| {
    if let Ok(event) = res {
        if event.kind.is_modify() || event.kind.is_create() {
            for path in event.paths {
                let _ = sender.send(path);
            }
        }
    }
}
```

The event kind is `Modify` (or `Create` if the editor used rename-based saving). The changed path (`plan.md`) is sent through the broadcast channel.

### Debouncing note

notify does not debounce events by default. A single save operation might generate multiple events (write + close, or create + modify). The broadcast channel handles this naturally:

- Multiple sends in quick succession are all delivered to receivers
- The JavaScript client calls `location.reload()` for each "reload" message
- The browser coalesces rapid reload requests (browsers do not queue reloads while a reload is in progress)

In practice, the user sees one reload per save, even if notify fires multiple events.

## Step 4: Broadcast to WebSocket Tasks

The broadcast sender delivers the path to all subscribed receivers. Each active WebSocket connection has its own receiver.

```
broadcast::Sender
    |
    +-- Receiver (WebSocket client 1) --> handle_ws task 1
    |
    +-- Receiver (WebSocket client 2) --> handle_ws task 2
    |
    +-- Receiver (WebSocket client 3) --> handle_ws task 3
```

If there are no receivers (no browsers connected), the send succeeds but the message is dropped. This is harmless.

## Step 5: WebSocket Sends "reload"

In each `handle_ws` task, the `tokio::select!` loop wakes up on the broadcast receive:

```rust
result = rx.recv() => {
    Ok(()) => {
        socket.send(Message::Text("reload".into())).await?;
    }
}
```

The WebSocket protocol frame is:
- Opcode: 1 (text frame)
- Payload: `"reload"` (6 bytes)

## Step 6: Browser Reloads

The JavaScript receives the message:

```javascript
ws.onmessage = function(event) {
    if (event.data === 'reload') {
        location.reload();
    }
};
```

`location.reload()` triggers a full page reload:

1. Browser sends `GET /view/plan.md` (or whatever page is currently displayed)
2. Server re-reads the file from disk (now with the updated content)
3. Server re-renders: comrak -> syntect -> flag spans -> template
4. Browser receives the fresh HTML and renders it

### Scroll position

`location.reload()` preserves scroll position in most browsers. The user's reading position is maintained across reloads, which is important for the editing workflow: you save a change, glance at the browser, and the updated content appears at the same position you were reading.

## Edge Cases

### Editor uses atomic save (rename)

Many editors (VS Code, vim with backup) save by writing to a temporary file and then renaming it over the original. This can produce `Create` events instead of `Modify` events. The watcher callback handles both:

```rust
if event.kind.is_modify() || event.kind.is_create() {
```

### Multiple files change simultaneously

If multiple files change at once (e.g., `git checkout` switches branches), the watcher fires events for each file. All events broadcast to WebSocket clients. The browser reloads once (browsers coalesce rapid reloads).

### Browser tab is not focused

The reload still happens. When the user switches back to the browser tab, they see the updated content.

### WebSocket disconnection and reconnection

If the WebSocket connection drops (network issue, server restart), the JavaScript auto-reconnects after 2 seconds:

```javascript
ws.onclose = function() {
    statusConnection.classList.add('disconnected');
    setTimeout(connectWebSocket, 2000);
};
```

During disconnection:
- The status bar shows a red disconnected indicator
- File changes are not delivered (no WebSocket to send to)
- When the connection re-establishes, the status bar turns green
- The page does NOT automatically reload on reconnect (this could be added but is not currently implemented)

If the server restarted, the browser page becomes stale. The user can manually reload or wait for the next file change to trigger a reload.

### Server restarts

If you stop and restart the server (ctrl-c then re-run `previewf serve`):

1. All WebSocket connections drop
2. JavaScript clients show disconnected status
3. After 2 seconds, clients try to reconnect
4. New server accepts the connections
5. Status indicator turns green
6. Next file change triggers a reload with fresh content

## Performance Characteristics

| Metric | Typical Value |
|--------|--------------|
| File change to OS notification | 10-50ms (platform dependent) |
| notify callback to broadcast | < 1ms |
| Broadcast to WebSocket send | < 1ms |
| WebSocket delivery to browser | < 5ms |
| Browser reload + server re-render | 50-100ms |
| **Total: file save to updated page** | **~100-150ms** |

This is fast enough that the browser update appears essentially instantaneous. Save in the editor, glance at the browser, see the change.

## The Broadcast Channel Architecture

The choice of `tokio::sync::broadcast` over alternatives:

| Alternative | Why Not |
|-------------|---------|
| `mpsc` (multi-producer, single-consumer) | Only one receiver. We need one per WebSocket client. |
| `watch` (single-value) | Only stores the latest value. We want to notify on every change, not just check current state. |
| `broadcast` | Multiple receivers, each gets every message, non-blocking send. Perfect fit. |

The broadcast channel capacity of 100 is generous. In practice, a fast typist saving every few seconds generates single-digit events per second. The 100-message buffer would only overflow if 100 files changed in rapid succession with no WebSocket clients consuming messages.
