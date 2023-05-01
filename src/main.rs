use std::fs::File;

use binrw::BinReaderExt;
use color_eyre::Result;

use toc::TocItem;
use tracing::*;

use crate::text::Page;

mod toc;
mod text;

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
    let mut text_dki = File::open("text.dki")?;

    let toc = toc::Toc::load(tree_dki, tree_dka)?;

    for page in &toc.entries {
        do_page(&mut text_dki, page)?;
    }

    Ok(())
}

fn do_page(mut f: &mut File, entry: &TocItem) -> Result<()> {
    let pages = text::Pages::load(&mut f, entry.page_number, entry.page_count)?;

    for page in pages.pages {
        println!("page {}: {:?}", page.number, page.lex());
    }

    for child in &entry.children {
        do_page(&mut f, child)?;
    }

    Ok(())
}
