use std::{fs::File, io::Cursor};

use binrw::BinReaderExt;
use color_eyre::Result;

use text::PageTable;
use tikv_jemallocator::Jemalloc;
use toc::TocItem;
use tracing::*;

#[global_allocator]
static GLOBAL: Jemalloc = Jemalloc;

use crate::text::Page;

mod toc;
mod text;
mod token;
mod typst;
mod decoding;

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

fn main() -> Result<()> {
    color_eyre::install()?;
    install_tracing()?;

    let tree_dki = File::open("tree.dki")?;
    let tree_dka = File::open("tree.dka")?;
    let text_dki = std::fs::read("text.dki")?;
    let mut text_dki = Cursor::new(text_dki.as_slice());

    let toc = toc::Toc::load(tree_dki, tree_dka)?;
    let page_table = text::PageTable::load(&mut text_dki)?;

    for page in &toc.entries {
        do_page(&mut text_dki, &page_table, page)?;
    }

    Ok(())
}

fn do_page(mut f: &mut Cursor<&[u8]>, page_table: &PageTable, entry: &TocItem) -> Result<()> {
    let pages = text::Pages::load(&mut f, page_table, entry.page_number, entry.page_count)?;

    for page in pages.pages {
        let lexed = page.lex();
        let mut out = String::new();

        typst::write_page(&page, &lexed, &mut out)?;

        println!("page {}: {}", page.number, out);
    }

    for child in &entry.children {
        do_page(&mut f, page_table, child)?;
    }

    Ok(())
}
