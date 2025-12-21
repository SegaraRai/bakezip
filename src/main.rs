use std::io;
use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand, ValueEnum};
use tokio::io::{AsyncReadExt, AsyncSeekExt};

use bakezip::zip::{
    compatibility::CompatibilityLevel,
    inspect::{EncodingSelectionStrategy, FieldSelectionStrategy, InspectConfig, InspectedArchive},
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

    /// Fallback encoding (default: auto-detect)
    #[arg(long, global = true)]
    encoding: Option<String>,

    /// Force specific encoding
    #[arg(long, global = true)]
    force_encoding: Option<String>,

    /// Ignore UTF-8 flag
    #[arg(long, global = true)]
    ignore_utf8_flag: bool,

    /// Ignore CRC32 mismatch in UTF-8 extra fields
    #[arg(long, global = true)]
    ignore_crc32_mismatch: bool,

    /// Field selection strategy
    #[arg(long, global = true, value_enum, default_value_t = FieldSelectionStrategyArg::CdhUnicodeThenLfhUnicodeThenCdh)]
    field_strategy: FieldSelectionStrategyArg,
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
    file: tokio::fs::File,
}

impl FileZipReader {
    async fn new(path: impl AsRef<Path>) -> io::Result<Self> {
        let file = tokio::fs::File::open(path).await?;
        Ok(Self { file })
    }
}

impl ZipReader for FileZipReader {
    async fn get_size(&mut self) -> io::Result<u64> {
        self.file.metadata().await.map(|m| m.len())
    }

    async fn read(&mut self, offset: u64, size: u64) -> io::Result<Vec<u8>> {
        self.file.seek(io::SeekFrom::Start(offset)).await?;
        let mut buffer = vec![0; size as usize];
        self.file.read_exact(&mut buffer).await?;
        Ok(buffer)
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    let mut reader = FileZipReader::new(&cli.input).await?;
    let zip_file = ZipFile::parse(&mut reader, |idx, err| {
        eprintln!("Warning at index {idx:?}: {err}");
        Ok(())
    })
    .await
    .map_err(|e| format!("Failed to parse zip: {e}"))?;

    let encoding_strategy = if let Some(enc) = cli.force_encoding {
        EncodingSelectionStrategy::ForceSpecified {
            encoding: enc,
            ignore_utf8_flag: cli.ignore_utf8_flag,
        }
    } else {
        EncodingSelectionStrategy::PreferOverallDetected {
            fallback_encoding: cli.encoding,
            ignore_utf8_flag: cli.ignore_utf8_flag,
        }
    };

    let config = InspectConfig {
        encoding: encoding_strategy,
        field_selection_strategy: cli.field_strategy.into(),
        ignore_crc32_mismatch: cli.ignore_crc32_mismatch,
        needs_original_bytes: false,
    };

    match cli.command.unwrap_or(Commands::Inspect) {
        Commands::Inspect => {
            let inspected = InspectedArchive::inspect(&zip_file, &config)
                .map_err(|e| format!("Failed to inspect zip: {}", e))?;

            let compatibility = CompatibilityLevel::analyze(&zip_file);
            println!("Compatibility: {compatibility:?}");

            println!("Overall encoding: {:?}", inspected.overall_encoding);
            println!("Entries: {}", inspected.entries.len());
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
        Commands::Rebuild { output, omit } => {
            let (chunks, _) = rebuild(&zip_file, &config, &omit)
                .map_err(|e| format!("Failed to rebuild zip: {e}"))?;

            let mut output_file = tokio::fs::File::create(output).await?;
            use tokio::io::AsyncWriteExt;

            for chunk in chunks {
                match chunk {
                    RebuildChunk::Binary(data) => {
                        output_file.write_all(&data).await?;
                    }
                    RebuildChunk::Reference { offset, size } => {
                        let data = reader.read(offset, size).await?;
                        output_file.write_all(&data).await?;
                    }
                }
            }
            println!("Rebuild complete.");
        }
    }

    Ok(())
}
