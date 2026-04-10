//! `hhkb` — command-line tool for HHKB Professional Hybrid keyboards.
//!
//! This binary exposes the `hhkb-core` library over a clap-based CLI,
//! talking to real hardware through the `hidapi-backend` feature.

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};
use hhkb_core::KeyboardMode;

mod commands;
mod format;

/// Top-level CLI definition.
#[derive(Debug, Parser)]
#[command(
    name = "hhkb",
    version,
    about = "Command-line tool for HHKB Professional Hybrid keyboards",
    propagate_version = true
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

/// All supported subcommands.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// List all attached HHKB devices.
    List,

    /// Print keyboard info (type, firmware, serial).
    Info,

    /// Print the current keyboard mode.
    Mode,

    /// Print the DIP switch states.
    Dipsw,

    /// Print all info plus the active keymap as a hex dump.
    Dump,

    /// Read the 128-byte keymap and output it as JSON.
    ReadKeymap {
        /// Keyboard mode to read the keymap for.
        #[arg(long, value_enum, default_value_t = ModeArg::Mac)]
        mode: ModeArg,

        /// Read the Fn layer instead of the base layer.
        #[arg(long = "fn", default_value_t = false)]
        fn_layer: bool,

        /// Write output to this file instead of stdout.
        #[arg(short = 'o', long = "output")]
        output: Option<PathBuf>,
    },

    /// Write a keymap from a JSON file. Prompts for confirmation.
    WriteKeymap {
        /// Path to the keymap JSON file to write.
        file: PathBuf,

        /// Keyboard mode to write the keymap for.
        #[arg(long, value_enum, default_value_t = ModeArg::Mac)]
        mode: ModeArg,

        /// Write to the Fn layer instead of the base layer.
        #[arg(long = "fn", default_value_t = false)]
        fn_layer: bool,

        /// Skip the interactive confirmation prompt.
        #[arg(short = 'y', long = "yes", default_value_t = false)]
        yes: bool,
    },

    /// Read the keymap and export it as a VIA-superset JSON profile.
    ExportVia {
        /// Write output to this file instead of stdout.
        #[arg(short = 'o', long = "output")]
        output: Option<PathBuf>,
    },

    /// Interactively discover which physical key corresponds to a keymap
    /// byte index. Temporarily overrides `keymap[index]` with a sentinel
    /// keycode (F1 = 0x3A by default), waits for the user to press keys,
    /// then restores the original keymap.
    DiscoverKey {
        /// Byte index into the 128-byte keymap to probe (0..=127). For
        /// the HHKB Pro Hybrid the 60 physical keys live in 1..=60.
        #[arg(long)]
        index: u8,

        /// Keyboard mode to read/write the keymap for.
        #[arg(long, value_enum, default_value_t = ModeArg::Mac)]
        mode: ModeArg,

        /// Probe the Fn layer instead of the base layer.
        #[arg(long = "fn", default_value_t = false)]
        fn_layer: bool,

        /// Sentinel HID keycode to temporarily assign to the index.
        /// Defaults to 0x3A (F1) — uncommonly used, easy to spot.
        #[arg(long, default_value_t = 0x3A)]
        sentinel: u8,

        /// Seconds to wait for the user to test the key before restoring.
        #[arg(long, default_value_t = 5)]
        wait_secs: u64,
    },

    /// Read the current keymap and print it as a labelled HHKB ANSI grid,
    /// so you can see what HID keycode sits at each byte index without
    /// modifying anything.
    DiscoverKeymapTable {
        /// Keyboard mode to read the keymap for.
        #[arg(long, value_enum, default_value_t = ModeArg::Mac)]
        mode: ModeArg,

        /// Read the Fn layer instead of the base layer.
        #[arg(long = "fn", default_value_t = false)]
        fn_layer: bool,
    },
}

/// Clap-friendly mirror of [`KeyboardMode`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ModeArg {
    /// HHK mode (layout 0).
    Hhk,
    /// Mac mode (layout 1).
    Mac,
    /// Lite mode (layout 2).
    Lite,
}

impl From<ModeArg> for KeyboardMode {
    fn from(m: ModeArg) -> Self {
        match m {
            ModeArg::Hhk => KeyboardMode::HHK,
            ModeArg::Mac => KeyboardMode::Mac,
            ModeArg::Lite => KeyboardMode::Lite,
        }
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("error: {err:#}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Command::List => commands::list(),
        Command::Info => commands::info(),
        Command::Mode => commands::mode(),
        Command::Dipsw => commands::dipsw(),
        Command::Dump => commands::dump(),
        Command::ReadKeymap {
            mode,
            fn_layer,
            output,
        } => commands::read_keymap(mode.into(), fn_layer, output.as_deref()),
        Command::WriteKeymap {
            file,
            mode,
            fn_layer,
            yes,
        } => commands::write_keymap(&file, mode.into(), fn_layer, yes),
        Command::ExportVia { output } => commands::export_via(output.as_deref()),
        Command::DiscoverKey {
            index,
            mode,
            fn_layer,
            sentinel,
            wait_secs,
        } => commands::discover_key(index, mode.into(), fn_layer, sentinel, wait_secs),
        Command::DiscoverKeymapTable { mode, fn_layer } => {
            commands::discover_keymap_table(mode.into(), fn_layer)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn cli_debug_assert() {
        Cli::command().debug_assert();
    }

    #[test]
    fn parses_list() {
        let cli = Cli::try_parse_from(["hhkb", "list"]).unwrap();
        assert!(matches!(cli.command, Command::List));
    }

    #[test]
    fn parses_info() {
        let cli = Cli::try_parse_from(["hhkb", "info"]).unwrap();
        assert!(matches!(cli.command, Command::Info));
    }

    #[test]
    fn parses_mode() {
        let cli = Cli::try_parse_from(["hhkb", "mode"]).unwrap();
        assert!(matches!(cli.command, Command::Mode));
    }

    #[test]
    fn parses_dipsw() {
        let cli = Cli::try_parse_from(["hhkb", "dipsw"]).unwrap();
        assert!(matches!(cli.command, Command::Dipsw));
    }

    #[test]
    fn parses_dump() {
        let cli = Cli::try_parse_from(["hhkb", "dump"]).unwrap();
        assert!(matches!(cli.command, Command::Dump));
    }

    #[test]
    fn parses_read_keymap_default() {
        let cli = Cli::try_parse_from(["hhkb", "read-keymap"]).unwrap();
        match cli.command {
            Command::ReadKeymap {
                mode,
                fn_layer,
                output,
            } => {
                assert_eq!(mode, ModeArg::Mac);
                assert!(!fn_layer);
                assert!(output.is_none());
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn parses_read_keymap_with_flags() {
        let cli = Cli::try_parse_from([
            "hhkb",
            "read-keymap",
            "--mode",
            "hhk",
            "--fn",
            "-o",
            "out.json",
        ])
        .unwrap();
        match cli.command {
            Command::ReadKeymap {
                mode,
                fn_layer,
                output,
            } => {
                assert_eq!(mode, ModeArg::Hhk);
                assert!(fn_layer);
                assert_eq!(output.as_deref(), Some(std::path::Path::new("out.json")));
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn parses_write_keymap() {
        let cli = Cli::try_parse_from([
            "hhkb",
            "write-keymap",
            "profile.json",
            "--mode",
            "mac",
            "--yes",
        ])
        .unwrap();
        match cli.command {
            Command::WriteKeymap {
                file,
                mode,
                fn_layer,
                yes,
            } => {
                assert_eq!(file, std::path::PathBuf::from("profile.json"));
                assert_eq!(mode, ModeArg::Mac);
                assert!(!fn_layer);
                assert!(yes);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn parses_write_keymap_fn_layer() {
        let cli =
            Cli::try_parse_from(["hhkb", "write-keymap", "profile.json", "--fn"]).unwrap();
        match cli.command {
            Command::WriteKeymap {
                file,
                mode,
                fn_layer,
                yes,
            } => {
                assert_eq!(file, std::path::PathBuf::from("profile.json"));
                assert_eq!(mode, ModeArg::Mac);
                assert!(fn_layer);
                assert!(!yes);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn parses_export_via() {
        let cli = Cli::try_parse_from(["hhkb", "export-via", "-o", "via.json"]).unwrap();
        match cli.command {
            Command::ExportVia { output } => {
                assert_eq!(output.as_deref(), Some(std::path::Path::new("via.json")));
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn write_keymap_requires_file() {
        assert!(Cli::try_parse_from(["hhkb", "write-keymap"]).is_err());
    }

    /// Exhaustiveness guard: if a new variant is added to [`Command`], this
    /// match must be updated — which reminds the author to wire it through
    /// the CLI dispatcher.
    #[test]
    fn subcommand_enum_exhaustive() {
        fn _assert(cmd: &Command) {
            match cmd {
                Command::List
                | Command::Info
                | Command::Mode
                | Command::Dipsw
                | Command::Dump
                | Command::ReadKeymap { .. }
                | Command::WriteKeymap { .. }
                | Command::ExportVia { .. }
                | Command::DiscoverKey { .. }
                | Command::DiscoverKeymapTable { .. } => {}
            }
        }
        _assert(&Command::List);
    }

    #[test]
    fn parses_discover_key_minimal() {
        let cli = Cli::try_parse_from(["hhkb", "discover-key", "--index", "60"]).unwrap();
        match cli.command {
            Command::DiscoverKey {
                index,
                mode,
                fn_layer,
                sentinel,
                wait_secs,
            } => {
                assert_eq!(index, 60);
                assert_eq!(mode, ModeArg::Mac);
                assert!(!fn_layer);
                assert_eq!(sentinel, 0x3A);
                assert_eq!(wait_secs, 5);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn parses_discover_key_with_flags() {
        let cli = Cli::try_parse_from([
            "hhkb",
            "discover-key",
            "--index",
            "30",
            "--mode",
            "hhk",
            "--fn",
            "--sentinel",
            "58",
            "--wait-secs",
            "10",
        ])
        .unwrap();
        match cli.command {
            Command::DiscoverKey {
                index,
                mode,
                fn_layer,
                sentinel,
                wait_secs,
            } => {
                assert_eq!(index, 30);
                assert_eq!(mode, ModeArg::Hhk);
                assert!(fn_layer);
                assert_eq!(sentinel, 58);
                assert_eq!(wait_secs, 10);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn discover_key_requires_index() {
        assert!(Cli::try_parse_from(["hhkb", "discover-key"]).is_err());
    }

    #[test]
    fn parses_discover_keymap_table_default() {
        let cli = Cli::try_parse_from(["hhkb", "discover-keymap-table"]).unwrap();
        match cli.command {
            Command::DiscoverKeymapTable { mode, fn_layer } => {
                assert_eq!(mode, ModeArg::Mac);
                assert!(!fn_layer);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn parses_discover_keymap_table_with_flags() {
        let cli = Cli::try_parse_from([
            "hhkb",
            "discover-keymap-table",
            "--mode",
            "lite",
            "--fn",
        ])
        .unwrap();
        match cli.command {
            Command::DiscoverKeymapTable { mode, fn_layer } => {
                assert_eq!(mode, ModeArg::Lite);
                assert!(fn_layer);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn mode_arg_maps_to_keyboard_mode() {
        assert_eq!(KeyboardMode::from(ModeArg::Hhk), KeyboardMode::HHK);
        assert_eq!(KeyboardMode::from(ModeArg::Mac), KeyboardMode::Mac);
        assert_eq!(KeyboardMode::from(ModeArg::Lite), KeyboardMode::Lite);
    }
}
