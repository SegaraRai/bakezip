use std::io::{self, Read, Seek, Write};
use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand, ValueEnum};

use bakezip::zip::{
    compatibility::CompatibilityLevel,
    inspect::{
        EncodingSelectionStrategy, FieldSelectionStrategy, InspectConfig, InspectedArchive,
        WaveDashHandling, WaveDashNormalization,
    },
    parse::{ZipFile, ZipReader},
    rebuild::{RebuildChunk, rebuild},
};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Input ZIP file
    input: PathBuf,

    /// Encoding to use (fallback by default, forced if --force is used)
    #[arg(long, short, global = true)]
    encoding: Option<String>,

    /// Force the specified encoding
    #[arg(long, short, global = true)]
    force: bool,

    /// Field selection strategy
    #[arg(long, short = 's', global = true, value_enum, default_value_t = FieldSelectionStrategyArg::CdhUnicodeThenLfhUnicodeThenCdh)]
    field: FieldSelectionStrategyArg,

    /// Ignore UTF-8 flag
    #[arg(long, global = true)]
    ignore_utf8_flag: bool,

    /// Ignore CRC32 mismatch in UTF-8 extra fields
    #[arg(long, global = true)]
    ignore_crc32_mismatch: bool,

    /// How to handle Wave Dash (U+301C) when decoding from Shift_JIS
    #[arg(long, global = true, value_enum, default_value_t = WaveDashHandlingArg::DecodeToFullwidthTilde)]
    wave_dash_handling: WaveDashHandlingArg,

    /// How to normalize Wave Dash (U+301C) and Fullwidth Tilde (U+FF5E)
    #[arg(long, global = true, value_enum, default_value_t = WaveDashNormalizationArg::Preserve)]
    wave_dash_normalization: WaveDashNormalizationArg,
}

#[derive(Subcommand)]
enum Commands {
    /// Inspect the archive (default)
    Inspect,
    /// Rebuild the archive
    Rebuild {
        /// Output file path
        #[arg(short, long)]
        output: PathBuf,

        /// Omit entries by index
        #[arg(long)]
        omit: Vec<u64>,

        /// Remove OS metadata files (__MACOSX, .DS_Store, Thumbs.db, desktop.ini)
        #[arg(long, short = 'm')]
        remove_os_metadata: bool,
    },
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum FieldSelectionStrategyArg {
    #[clap(name = "cdhu-lfhu-cdh")]
    CdhUnicodeThenLfhUnicodeThenCdh,
    #[clap(name = "cdhu-lfhu-lfh")]
    CdhUnicodeThenLfhUnicodeThenLfh,
    #[clap(name = "lfhu-cdhu-cdh")]
    LfhUnicodeThenCdhUnicodeThenCdh,
    #[clap(name = "lfhu-cdhu-lfh")]
    LfhUnicodeThenCdhUnicodeThenLfh,
    #[clap(name = "cdhu-cdh")]
    CdhUnicodeThenCdh,
    #[clap(name = "cdh")]
    CdhOnly,
    #[clap(name = "lfhu-lfh")]
    LfhUnicodeThenLfh,
    #[clap(name = "lfh")]
    LfhOnly,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum WaveDashHandlingArg {
    DecodeToFullwidthTilde,
    DecodeToWaveDash,
}

impl From<WaveDashHandlingArg> for WaveDashHandling {
    fn from(arg: WaveDashHandlingArg) -> Self {
        match arg {
            WaveDashHandlingArg::DecodeToFullwidthTilde => WaveDashHandling::DecodeToFullwidthTilde,
            WaveDashHandlingArg::DecodeToWaveDash => WaveDashHandling::DecodeToWaveDash,
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum WaveDashNormalizationArg {
    Preserve,
    NormalizeToFullwidthTilde,
    NormalizeToWaveDash,
}

impl From<WaveDashNormalizationArg> for WaveDashNormalization {
    fn from(arg: WaveDashNormalizationArg) -> Self {
        match arg {
            WaveDashNormalizationArg::Preserve => WaveDashNormalization::Preserve,
            WaveDashNormalizationArg::NormalizeToFullwidthTilde => {
                WaveDashNormalization::NormalizeToFullwidthTilde
            }
            WaveDashNormalizationArg::NormalizeToWaveDash => {
                WaveDashNormalization::NormalizeToWaveDash
            }
        }
    }
}

impl From<FieldSelectionStrategyArg> for FieldSelectionStrategy {
    fn from(arg: FieldSelectionStrategyArg) -> Self {
        match arg {
            FieldSelectionStrategyArg::CdhUnicodeThenLfhUnicodeThenCdh => {
                FieldSelectionStrategy::CdhUnicodeThenLfhUnicodeThenCdh
            }
            FieldSelectionStrategyArg::CdhUnicodeThenLfhUnicodeThenLfh => {
                FieldSelectionStrategy::CdhUnicodeThenLfhUnicodeThenLfh
            }
            FieldSelectionStrategyArg::LfhUnicodeThenCdhUnicodeThenCdh => {
                FieldSelectionStrategy::LfhUnicodeThenCdhUnicodeThenCdh
            }
            FieldSelectionStrategyArg::LfhUnicodeThenCdhUnicodeThenLfh => {
                FieldSelectionStrategy::LfhUnicodeThenCdhUnicodeThenLfh
            }
            FieldSelectionStrategyArg::CdhUnicodeThenCdh => {
                FieldSelectionStrategy::CdhUnicodeThenCdh
            }
            FieldSelectionStrategyArg::CdhOnly => FieldSelectionStrategy::CdhOnly,
            FieldSelectionStrategyArg::LfhUnicodeThenLfh => {
                FieldSelectionStrategy::LfhUnicodeThenLfh
            }
            FieldSelectionStrategyArg::LfhOnly => FieldSelectionStrategy::LfhOnly,
        }
    }
}

struct FileZipReader {
    file: std::fs::File,
}

impl FileZipReader {
    fn new(path: impl AsRef<Path>) -> io::Result<Self> {
        let file = std::fs::File::open(path)?;
        Ok(Self { file })
    }
}

impl ZipReader for FileZipReader {
    async fn get_size(&mut self) -> io::Result<u64> {
        let metadata = self.file.metadata()?;
        Ok(metadata.len())
    }

    async fn read(&mut self, offset: u64, size: u64) -> io::Result<Vec<u8>> {
        self.file.seek(io::SeekFrom::Start(offset))?;
        let mut buffer = vec![0; size as usize];
        self.file.read_exact(&mut buffer)?;
        Ok(buffer)
    }
}

fn is_os_metadata_file(filename: &str) -> bool {
    let lower = filename.to_lowercase();
    // Directories and files
    if lower.starts_with("__macosx/") || lower.contains("/__macosx/") {
        return true;
    }
    if lower == ".ds_store"
        || lower.starts_with(".ds_store/")
        || lower.ends_with("/.ds_store")
        || lower.contains("/.ds_store/")
    {
        return true;
    }

    // Files only
    if lower == "thumbs.db" || lower.ends_with("/thumbs.db") {
        return true;
    }
    if lower == "desktop.ini" || lower.ends_with("/desktop.ini") {
        return true;
    }

    false
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    let mut reader = FileZipReader::new(&cli.input)?;
    let zip_file = ZipFile::parse(&mut reader, |idx, err| {
        eprintln!("Warning at index {idx:?}: {err}");
        Ok(())
    })
    .await
    .map_err(|e| format!("Failed to parse zip: {e}"))?;

    let encoding_strategy = if cli.force {
        if let Some(enc) = cli.encoding {
            EncodingSelectionStrategy::ForceSpecified {
                encoding: enc,
                ignore_utf8_flag: cli.ignore_utf8_flag,
            }
        } else {
            return Err("You must specify --encoding when using --force".into());
        }
    } else {
        EncodingSelectionStrategy::PreferOverallDetected {
            fallback_encoding: cli.encoding,
            ignore_utf8_flag: cli.ignore_utf8_flag,
        }
    };

    let config = InspectConfig {
        encoding: encoding_strategy,
        field_selection_strategy: cli.field.into(),
        ignore_crc32_mismatch: cli.ignore_crc32_mismatch,
        needs_original_bytes: false,
        wave_dash_handling: cli.wave_dash_handling.into(),
        wave_dash_normalization: cli.wave_dash_normalization.into(),
    };

    match cli.command.unwrap_or(Commands::Inspect) {
        Commands::Inspect => {
            let inspected = InspectedArchive::inspect(&zip_file, &config)
                .map_err(|e| format!("Failed to inspect zip: {}", e))?;

            println!("Overall encoding: {:?}", inspected.overall_encoding);

            let compatibility = CompatibilityLevel::analyze(&zip_file);
            println!("Compatibility: {compatibility:?}");

            println!("Entries: {}", inspected.entries.len());
            if inspected.contains_sjis_wave_dash {
                println!("Contains Shift_JIS Wave Dash/Fullwidth Tilde");
            }
            if inspected.contains_other_wave_dash {
                println!("Contains Wave Dash (Non-Shift_JIS)");
            }
            if inspected.contains_other_fullwidth_tilde {
                println!("Contains Fullwidth Tilde (Non-Shift_JIS)");
            }

            for (i, entry) in inspected.entries.iter().enumerate() {
                let filename = entry
                    .filename
                    .decoded
                    .as_ref()
                    .map(|d| d.string.as_str())
                    .unwrap_or("<decoding failed>");

                println!("{i}: {filename}");
            }
        }
        Commands::Rebuild {
            output,
            omit,
            remove_os_metadata,
        } => {
            let mut omit_indices = omit.clone();
            if remove_os_metadata {
                let inspected = InspectedArchive::inspect(&zip_file, &config)
                    .map_err(|e| format!("Failed to inspect zip for filtering: {}", e))?;
                for (i, entry) in inspected.entries.iter().enumerate() {
                    let filename = entry
                        .filename
                        .decoded
                        .as_ref()
                        .map(|d| d.string.as_str())
                        .unwrap_or("");
                    if is_os_metadata_file(filename) {
                        omit_indices.push(i as u64);
                    }
                }
            }

            let (chunks, _) = rebuild(&zip_file, &config, &omit_indices)
                .map_err(|e| format!("Failed to rebuild zip: {e}"))?;

            let mut output_file = std::fs::File::create(output)?;
            for chunk in chunks {
                match chunk {
                    RebuildChunk::Binary(data) => {
                        output_file.write_all(&data)?;
                    }
                    RebuildChunk::Reference { offset, size } => {
                        let data = reader.read(offset, size).await?;
                        output_file.write_all(&data)?;
                    }
                }
            }

            println!("Rebuild complete.");
        }
    }

    Ok(())
}
