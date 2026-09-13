use std::borrow::Cow;
use std::error;
use std::fmt;
use std::fs::read_to_string;
use std::path::Path;
use std::path::PathBuf;

use clap::Parser;
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
    report(&generator, &fast_layout);
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

fn report(generator: &Oxeylyzer, layout: &FastLayout) {
    print_layout(layout);

    let stats = generator.get_layout_stats(&layout);

    println!("Total SFB penalty: {:.3}", stats.sfb);
    println!(
        "DSFB penalty (distance 1/2/3): {:.3}/{:.3}/{:.3}",
        stats.dsfb, stats.dsfb2, stats.dsfb3
    );
    println!("Scissors: {:.3}", stats.scissors);
    println!("LSBS penalty: {:.3}", stats.lsbs);
    println!("Excessive stretches penalty : {:.3}", stats.stretches);
    println!("Pinky-ring penalty: {:.3}", stats.pinky_ring);
    println!("Fspeed penalty: {:.3}", stats.fspeed);
    println!("Fspeed per finger:");
    for (finger, fspeed) in stats.finger_speed.iter().enumerate() {
        println!("\tFinger {finger}: {fspeed:.3}");
    }

    println!("Trigrams:");
    println!("\tAlternates: {:.3}", stats.trigram_stats.alternates);
    println!(
        "\tAlternates (same-finger skip): {:.3}",
        stats.trigram_stats.alternates_sfs
    );
    println!("\tInrolls: {:.3}", stats.trigram_stats.inrolls);
    println!("\tOutrolls: {:.3}", stats.trigram_stats.outrolls);
    println!("\tOnehands: {:.3}", stats.trigram_stats.onehands);
    println!("\tRedirects: {:.3}", stats.trigram_stats.redirects);
    println!(
        "\tRedirects (same-finger skip): {:.3}",
        stats.trigram_stats.redirects_sfs
    );
    println!("\tBad redirects: {:.3}", stats.trigram_stats.bad_redirects);
    println!(
        "\tBad redirects (same-finger skip): {:.3}",
        stats.trigram_stats.bad_redirects_sfs
    );
    println!("\tSFBs: {:.3}", stats.trigram_stats.sfbs);
    println!("\tBad SFBs: {:.3}", stats.trigram_stats.bad_sfbs);
    println!("\tSFTs: {:.3}", stats.trigram_stats.sfts);
    println!("\tThumbs: {:.3}", stats.trigram_stats.thumbs);
    println!("\tOther: {:.3}", stats.trigram_stats.other);
    println!("\tInvalid: {:.3}", stats.trigram_stats.invalid);

    println!("Total score: {}", stats.score);
}

fn print_layout(layout: &FastLayout) {
    let mut iter = layout.keys.iter();
    let shape = layout.shape.inner();
    let max_len = shape.iter().max().copied().unwrap_or(0);

    for &l in layout.shape.inner().iter() {
        let gap = max_len - l;
        let padding: Cow<str> = if gap > 0 {
            Cow::Owned(std::iter::repeat(' ').take(gap / 2).collect::<String>())
        } else {
            Cow::Borrowed("")
        };

        let mut i = 0;
        print!("{padding}");
        for u in iter.by_ref() {
            let c = layout.mapping.get_c(*u);
            print!("{c}");

            i += 1;

            if l == i {
                break;
            }
        }
        println!("{padding}");
    }
    println!(
        "{}",
        std::iter::repeat('-').take(max_len).collect::<String>()
    )
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
