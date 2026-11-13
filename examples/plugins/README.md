# Example plugins

Each folder is a complete plugin. Try them all without copying anything:

```bash
ZENITH_PLUGIN_PATH="$PWD/examples/plugins" ZENITH_PLUGIN_DEV=1 cargo run --release
```

Or press *Install in Zenith* on the example's page on the website: they are
built into Zenith and install live. Copying a folder into
`%APPDATA%\Zenith\plugins\` works too; with dev mode on in Settings, Zenith
picks it up without a restart.

| Plugin | Language | Shows |
|---|---|---|
| [counter](counter) | Python | The basics: a view, button events, a pill badge, saving state |
| [todo](todo) | Python | A page with an input, chips and a list; header actions, a hotkey, dropped files, copy to clipboard |
| [disk-space](disk-space) | PowerShell | Poll mode: a script that prints JSON and exits. Runs on any Windows machine with nothing installed |
| [analog-clock](analog-clock) | Node.js | Canvas drawing, live updates every second, click events with coordinates, a larger page |

`zenith.py` and `zenith.js` in these folders are copies of the helpers in
[sdk/](../../sdk). The full guide is [docs/plugins.html](../../docs/plugins.html).
