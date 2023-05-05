use std::{fs::File, io::Cursor, path::PathBuf};

use binrw::BinReaderExt;
use clap::Parser;
use color_eyre::{Help, Result, SectionExt};
use for_flutter_encoder::Segment;
use ormlite::{sqlite::{SqliteConnectOptions, SqliteConnection}, ConnectOptions, Connection, Executor, Model};
use prost::Message;
use text::PageTable;
use tikv_jemallocator::Jemalloc;
use toc::TocItem;
use tracing::*;

#[global_allocator]
static GLOBAL: Jemalloc = Jemalloc;

mod decoding;
mod encoder;
mod for_flutter_encoder;
mod text;
mod toc;
mod token;
mod typst;
mod for_flutter_proto;

#[derive(Parser)]
struct Opts {
    #[clap(short, long)]
    data_dir: PathBuf,

    #[clap(short, long)]
    out_file: PathBuf,
}

fn install_tracing() -> Result<()> {
    use tracing_subscriber::fmt::format::FmtSpan;
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;

    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_span_events(FmtSpan::NEW | FmtSpan::CLOSE)
        .pretty();
    let filter_layer = tracing_subscriber::EnvFilter::builder()
        .with_default_directive(concat!(env!("CARGO_CRATE_NAME"), "=debug").parse()?)
        .from_env()?;

    tracing_subscriber::registry()
        .with(tracing_error::ErrorLayer::default())
        .with(filter_layer)
        .with(fmt_layer)
        .init();

    Ok(())
}

#[derive(ormlite::Model, Debug)]
pub struct Page {
    id: u32,
    content: Vec<u8>,
    plain: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let opts = Opts::parse();

    color_eyre::install()?;
    install_tracing()?;

    let tree_dki = File::open(opts.data_dir.join("tree.dki"))?;
    let tree_dka = File::open(opts.data_dir.join("tree.dka"))?;
    let text_dki = std::fs::read(opts.data_dir.join("text.dki"))?;
    let mut text_dki = Cursor::new(text_dki.as_slice());

    let toc = toc::Toc::load(tree_dki, tree_dka)?;
    let page_table = text::PageTable::load(&mut text_dki)?;

    let mut conn =
        SqliteConnectOptions::new()
            .filename(&opts.out_file)
            .journal_mode(ormlite::sqlite::SqliteJournalMode::Off)
            .synchronous(ormlite::sqlite::SqliteSynchronous::Off)
            .row_buffer_size(100000)
            .locking_mode(ormlite::sqlite::SqliteLockingMode::Exclusive)
            .create_if_missing(true)
            .connect().await?;


    ormlite::query(r#"
PRAGMA temp_store = MEMORY;
    
CREATE TABLE page (
  id INTEGER not null primary key,
  content BLOB not null,
  plain TEXT not null
);

CREATE VIRTUAL TABLE page_fts USING fts5(
    plain,
    content='page',
    content_rowid='id'
);

CREATE TRIGGER page_ai AFTER INSERT ON page
    BEGIN
        INSERT INTO page_fts (rowid, plain)
        VALUES (new.id, new.plain);
    END;
   "#).execute(&mut conn).await?;

    for page in &toc.entries {
        do_page(&mut text_dki, &page_table, page, &mut conn).await?;
    }

    Ok(())
}

#[async_recursion::async_recursion]
async fn do_page(mut f: &mut Cursor<&[u8]>, page_table: &PageTable, entry: &TocItem, conn: &mut SqliteConnection) -> Result<()> {
    let pages = text::Pages::load(&mut f, page_table, entry.page_number, entry.page_count)?;

    for (i, page) in pages.pages.iter().enumerate() {
        let lexed = page.lex();
        let mut e = for_flutter_encoder::ForFlutter::new();

        encoder::encode_page(entry, entry.page_number + i, &lexed, &mut e)?;

        Page {
            id: (entry.page_number + i) as u32,
            plain: e.plain.to_owned(),
            content: e.to_proto().encode_to_vec(),
        }.insert(&mut *conn).await?;
    }

    for child in &entry.children {
        do_page(&mut f, page_table, child, conn).await?;
    }

    Ok(())
}
