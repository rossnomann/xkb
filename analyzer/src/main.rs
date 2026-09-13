use std::error;
use std::fmt;
use std::fs::read_to_string;
use std::path::Path;
use std::path::PathBuf;

use clap::Parser;
use comfy_table::presets::UTF8_FULL;
use comfy_table::{Row, Table};
use libdof::Dof;
use libdof::Keyboard;
use libdof::dofinitions::Key;
use oxeylyzer_core::fast_layout::FastLayout;
use oxeylyzer_core::{
    corpus_cleaner::{CleanCorpus, CorpusCleaner},
    data::Data,
    generate::Oxeylyzer,
    layout::Layout,
    weights::Config,
};

#[derive(Debug, Parser)]
#[command(version)]
struct Args {
    #[arg(short)]
    config: PathBuf,
    #[arg(long)]
    raw_corpus: Option<PathBuf>,
    #[arg(long)]
    layout: Option<String>,
    #[arg(long)]
    pins: Option<String>,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("{err}");
        std::process::exit(1)
    }
}

fn run() -> Result<(), Error> {
    let args = Args::parse();
    let config = Config::with_loaded_weights(args.config).map_err(Error::load_config)?;
    let layout_path = find_layout_path(
        &config,
        args.layout.as_ref().map_or("default.dof", |x| x.as_str()),
    )?;
    let dof_layout = load_dof_layout(layout_path)?;
    let data = load_corpus(&config, args.raw_corpus, &dof_layout)?;
    let generator = Oxeylyzer::new(data, config);
    let base_layout = Layout::from(dof_layout);
    let pins = map_pins(&base_layout, args.pins.as_ref().map_or("", |x| x.as_str()));
    let fast_layout =
        generator.fast_layout(&base_layout, pins.as_ref().map_or(&[], |x| x.as_slice()));
    report_layout(&generator, &fast_layout);
    Ok(())
}

fn load_corpus(
    config: &Config,
    raw: Option<impl AsRef<Path>>,
    layout: &Dof,
) -> Result<Data, Error> {
    if config.corpus.exists() {
        Data::load(config.corpus.as_path()).map_err(Error::load_corpus)
    } else {
        let raw = raw.ok_or_else(Error::raw_corpus_path_required)?;
        let cleaner = create_corpus_cleaner(layout);
        let data = create_corpus(raw, &cleaner)?;
        data.save(
            config
                .corpus
                .parent()
                .ok_or_else(Error::corpus_root_not_found)?,
        )
        .map_err(Error::save_corpus)?;
        Ok(data)
    }
}

fn create_corpus_cleaner(layout: &Dof) -> CorpusCleaner {
    let alphabet = layout.layers().iter().flat_map(|(_name, layer)| {
        layer.inner().iter().flat_map(|keys| {
            keys.iter().filter_map(|key| {
                if let Key::Char(c) = key {
                    Some(*c)
                } else {
                    None
                }
            })
        })
    });
    let uppercase_mappings = layout
        .main_layer()
        .inner()
        .iter()
        .flatten()
        .zip(layout.shift_layer().inner().iter().flatten())
        .filter_map(|(main, shift)| {
            if let (Key::Char(a), Key::Char(b)) = (main, shift) {
                Some((*a, *b))
            } else {
                None
            }
        });
    CorpusCleaner::builder()
        .with_chars(alphabet)
        .with_uppercase_mappings(uppercase_mappings)
        .build()
}

fn create_corpus(source: impl AsRef<Path>, corpus_cleaner: &CorpusCleaner) -> Result<Data, Error> {
    let source = source.as_ref();
    let stem = source.file_stem().and_then(|x| x.to_str()).unwrap_or("new");
    let raw_corpus = read_to_string(source).map_err(Error::read_raw_corpus)?;
    let mut data: Data = raw_corpus
        .chars()
        .clean_corpus(corpus_cleaner)
        .flatten()
        .collect();
    data.name = stem.to_owned();
    Ok(data)
}

fn find_layout_path(config: &Config, name: &str) -> Result<PathBuf, Error> {
    let basename = Path::new(name);
    config
        .layouts
        .iter()
        .filter_map(|path| {
            let p = path.join(basename);
            if p.exists() { Some(p) } else { None }
        })
        .next()
        .ok_or_else(Error::layout_not_found)
}

fn load_dof_layout(path: impl AsRef<Path>) -> Result<Dof, Error> {
    let buf = std::fs::read_to_string(&path).map_err(Error::read_dof_layout)?;
    serde_json::from_str(&buf).map_err(Error::read_dof_layout)
}

fn map_pins(layout: &Layout, raw: &str) -> Option<Vec<usize>> {
    if raw.is_empty() {
        return None;
    }
    let chars: Vec<char> = raw.chars().collect();
    let pins = layout
        .keys
        .iter()
        .enumerate()
        .filter_map(|(i, c)| if chars.contains(c) { Some(i) } else { None })
        .collect();
    Some(pins)
}

fn report_layout(generator: &Oxeylyzer, layout: &FastLayout) {
    print_layout(layout);

    let stats = generator.get_layout_stats(&layout);

    let mut table = Table::new();
    table
        .load_style(UTF8_FULL)
        .set_header(["Stats", "", "Trigrams", ""])
        .add_rows([
            vrow_f64(
                "SFB",
                stats.sfb,
                "Alternates",
                stats.trigram_stats.alternates,
            ),
            vrow_f64(
                "DSFB",
                stats.dsfb,
                "Alternates SFS",
                stats.trigram_stats.alternates_sfs,
            ),
            vrow_f64("DSFB2", stats.dsfb2, "Inrolls", stats.trigram_stats.inrolls),
            vrow_f64(
                "DSFB3",
                stats.dsfb3,
                "Outrolls",
                stats.trigram_stats.outrolls,
            ),
            vrow_f64(
                "Scissors",
                stats.scissors,
                "Onehands",
                stats.trigram_stats.onehands,
            ),
            vrow_f64("LSBS", stats.lsbs, "Redirs", stats.trigram_stats.redirects),
            vrow_f64(
                "Stretches",
                stats.stretches,
                "Redirs SFS",
                stats.trigram_stats.redirects_sfs,
            ),
            vrow_f64(
                "Pinky ring",
                stats.pinky_ring,
                "Bad redirs",
                stats.trigram_stats.bad_redirects,
            ),
            vrow_f64(
                "Fspeed",
                stats.fspeed,
                "Bad redirs SFS",
                stats.trigram_stats.bad_redirects_sfs,
            ),
        ])
        .add_rows([
            [
                "".into(),
                "".into(),
                "SFBs".into(),
                format_f64(stats.trigram_stats.sfbs),
            ],
            [
                "".into(),
                "".into(),
                "Bad SFBs".into(),
                format_f64(stats.trigram_stats.bad_sfbs),
            ],
            [
                "".into(),
                "".into(),
                "SFTs".into(),
                format_f64(stats.trigram_stats.sfts),
            ],
            [
                "".into(),
                "".into(),
                "Thumbs".into(),
                format_f64(stats.trigram_stats.thumbs),
            ],
            [
                "".into(),
                "".into(),
                "Other".into(),
                format_f64(stats.trigram_stats.other),
            ],
            [
                "".into(),
                "".into(),
                "Invalid".into(),
                format_f64(stats.trigram_stats.invalid),
            ],
        ])
        .add_row(["Total score".into(), format!("{}", stats.score)]);
    println!("{table}");

    let mut table = Table::new();
    table.load_style(UTF8_FULL);
    table.set_header(0..stats.finger_speed.len());
    table.add_row(stats.finger_speed.iter().map(|x| format_f64(*x)));
    println!("Fspeed per finger:\n{table}");
}

fn vrow_f64(label_a: &str, value_a: f64, label_b: &str, value_b: f64) -> Row {
    [
        label_a.into(),
        format_f64(value_a),
        label_b.into(),
        format_f64(value_b),
    ]
    .into()
}

fn format_f64(value: f64) -> String {
    format!("{value:.3}")
}

fn print_layout(layout: &FastLayout) {
    let mut iter = layout.keys.iter();
    let shape = layout.shape.inner();
    let max_len = shape.iter().max().copied().unwrap_or(0);
    let mut table = Table::new();
    table.load_style(UTF8_FULL);

    for &l in layout.shape.inner().iter() {
        let mut cells = vec![];
        let gap = max_len - l;
        let padding: Vec<char> = if gap > 0 {
            std::iter::repeat(' ').take(gap / 2).collect()
        } else {
            vec![]
        };
        cells.extend(&padding);
        let mut i = 0;
        for u in iter.by_ref() {
            let c = layout.mapping.get_c(*u);
            cells.push(c);
            i += 1;
            if l == i {
                break;
            }
        }
        cells.extend(padding);
        table.add_row(cells);
    }
    println!("{table}")
}

#[derive(Clone, Debug)]
struct Error(String);

impl Error {
    fn corpus_root_not_found() -> Self {
        Self(String::from("Could not find corpus root"))
    }

    fn layout_not_found() -> Self {
        Self(String::from("Unable to find a layout"))
    }

    fn load_config(source: impl fmt::Display) -> Self {
        Self(format!("Could not load config: {}", source))
    }

    fn load_corpus(source: impl fmt::Display) -> Self {
        Self(format!("Could not load corpus: {}", source))
    }

    fn raw_corpus_path_required() -> Self {
        Self(String::from("Raw corpus path is required"))
    }

    fn read_dof_layout(source: impl fmt::Display) -> Self {
        Self(format!("Failed to read dof layout: {}", source))
    }

    fn read_raw_corpus(source: impl fmt::Display) -> Self {
        Self(format!("Failed to read raw corpus: {}", source))
    }

    fn save_corpus(source: impl fmt::Display) -> Self {
        Self(format!("Could not save corpus: {}", source))
    }
}

impl fmt::Display for Error {
    fn fmt(&self, out: &mut fmt::Formatter) -> fmt::Result {
        write!(out, "{}", self.0)
    }
}

impl error::Error for Error {}
