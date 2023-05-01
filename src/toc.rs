use binrw::BinReaderExt;
use std::{
    io::{BufRead, BufReader, Read},
    iter::Peekable,
};

use encoding_rs::WINDOWS_1252;
use encoding_rs_io::DecodeReaderBytesBuilder;

pub struct Toc {
    pub entries: Vec<TocItem>,
}

impl Toc {
    pub fn load(tree_dki: impl Read, mut tree_dka: impl BinReaderExt) -> eyre::Result<Self> {
        let tree_dki = DecodeReaderBytesBuilder::new()
            .encoding(Some(WINDOWS_1252))
            .build(tree_dki);

        let lines = BufReader::new(tree_dki)
            .lines()
            .collect::<Result<Vec<_>, _>>()?;

        let page_numbers = Self::load_page_numbers(&mut tree_dka)?;

        assert_eq!(lines.len(), page_numbers.block.len());

        let toc = Self::ingest(lines, page_numbers.block);

        Ok(Toc { entries: toc })
    }

    fn load_page_numbers<R: BinReaderExt>(dka: &mut R) -> eyre::Result<DkaBlock> {
        dbg!(dka.read_le::<DkaBlock>()?.len);
        dbg!(dka.read_le::<DkaBlock>()?.len);
        dbg!(dka.read_le::<DkaBlock>()?.len);
        Ok(dka.read_le::<DkaBlock>()?)
    }

    fn ingest(lines: Vec<String>, page_numbers: Vec<i32>) -> Vec<TocItem> {
        let it = lines.into_iter().enumerate().map(|(i, line)| {
            let trimmed = line.trim_start();
            let level = (line.len() - trimmed.len()) + 1;
            let page_number = if i == 0 { 1 } else { page_numbers[i - 1] };
            TocItem {
                id: i,
                title: trimmed.to_owned(),
                level: level as u8,
                page_number: page_number as usize,
                page_count: (page_numbers[i] - page_number) as usize,
                children: Vec::new(),
            }
        });

        let toc = Self::build_toc_item(0, &mut it.peekable());

        toc
    }

    fn build_toc_item(
        level: u8,
        rest: &mut Peekable<impl Iterator<Item = TocItem>>,
    ) -> Vec<TocItem> {
        let mut children = Vec::new();

        loop {
            let Some(mut next) = rest.next_if(|next| level < next.level) else { return children; };

            next.children = Self::build_toc_item(next.level, rest);
            children.push(next);
        }
    }
}

#[derive(Debug)]
pub struct TocItem {
    pub id: usize,
    pub title: String,
    pub level: u8,
    pub page_number: usize,
    pub page_count: usize,
    pub children: Vec<TocItem>,
}

#[derive(Debug)]
#[binrw::binrw]
#[br(little)]
struct DkaBlock {
    #[br(map = |x: u32| x + 1)]
    len: u32,

    #[br(count = len)]
    block: Vec<i32>,
}
