//! The example plugins, built into Zenith so they install offline
//! (`zenith://install?example=<id>`).

/// A plugin's files: (relative path, contents).
pub type Files = &'static [(&'static str, &'static [u8])];

/// (plugin id, its files).
pub static EXAMPLES: &[(&str, Files)] = &[
    (
        "counter",
        &[
            (
                "main.py",
                include_bytes!("../../../../examples/plugins/counter/main.py"),
            ),
            (
                "zenith.py",
                include_bytes!("../../../../examples/plugins/counter/zenith.py"),
            ),
            (
                "plugin.toml",
                include_bytes!("../../../../examples/plugins/counter/plugin.toml"),
            ),
        ],
    ),
    (
        "todo",
        &[
            (
                "main.py",
                include_bytes!("../../../../examples/plugins/todo/main.py"),
            ),
            (
                "zenith.py",
                include_bytes!("../../../../examples/plugins/todo/zenith.py"),
            ),
            (
                "plugin.toml",
                include_bytes!("../../../../examples/plugins/todo/plugin.toml"),
            ),
        ],
    ),
    (
        "disk-space",
        &[
            (
                "disk.ps1",
                include_bytes!("../../../../examples/plugins/disk-space/disk.ps1"),
            ),
            (
                "plugin.toml",
                include_bytes!("../../../../examples/plugins/disk-space/plugin.toml"),
            ),
        ],
    ),
    (
        "analog-clock",
        &[
            (
                "main.js",
                include_bytes!("../../../../examples/plugins/analog-clock/main.js"),
            ),
            (
                "zenith.js",
                include_bytes!("../../../../examples/plugins/analog-clock/zenith.js"),
            ),
            (
                "plugin.toml",
                include_bytes!("../../../../examples/plugins/analog-clock/plugin.toml"),
            ),
        ],
    ),
];

pub fn find(id: &str) -> Option<Files> {
    EXAMPLES
        .iter()
        .find(|(e, _)| *e == id)
        .map(|(_, files)| *files)
}
