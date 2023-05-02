use binrw::{BinRead, BinReaderExt, VecArgs};
use std::{
    fmt::Debug,
    io::{BufRead, BufReader, Read},
    iter::Peekable,
};

use encoding_rs::WINDOWS_1252;
use encoding_rs_io::DecodeReaderBytesBuilder;

use crate::token::Token;

pub struct PageTable {
    table: Vec<i32>,
    has_magic: bool,
}

impl PageTable {
    pub fn load(mut text_dki: impl BinReaderExt) -> eyre::Result<Self> {
        text_dki.seek(std::io::SeekFrom::Start(0))?;
        let magic = text_dki.read_le::<u32>()?;
        let has_magic = if magic == 0x1924cc {
            let version = text_dki.read_le::<i32>()?;
            true
        } else {
            text_dki.seek(std::io::SeekFrom::Start(0))?;
            false
        };

        let page_table = text_dki.read_le::<DkaBlock>()?.block;

        Ok(PageTable {
            table: page_table,
            has_magic,
        })
    }
}

pub struct Pages {
    pub start: usize,
    pub pages: Vec<Page>,
}

impl Pages {
    pub fn load(
        mut text_dki: impl BinReaderExt,
        page_table: &PageTable,
        page_number: usize,
        count: usize,
    ) -> eyre::Result<Self> {
        let pages = (page_number..(page_number + count))
            .map(|i| Self::load_page(&mut text_dki, &page_table.table, i, page_table.has_magic))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Pages {
            start: page_number,
            pages,
        })
    }

    fn load_page(
        mut text_dki: impl BinReaderExt,
        page_table: &[i32],
        page_number: usize,
        has_magic: bool,
    ) -> eyre::Result<Page> {
        let address = page_table[page_number - 1];
        text_dki.seek(std::io::SeekFrom::Start(address as u64))?;
        let page_size = text_dki.read_le::<u16>()?;
        let (atom_count, word_count, page_size) = if has_magic {
            (
                text_dki.read_le::<u16>()?,
                text_dki.read_le::<u16>()?,
                page_size,
            )
        } else {
            (0, 0, page_size - 2)
        };
        let data = text_dki
            .read_le_args::<Vec<u8>>(VecArgs::builder().count(page_size as usize).finalize())?;

        Ok(Page {
            number: page_number,
            atom_count,
            word_count,
            data,
        })
    }
}

#[derive(Debug)]
pub struct Page {
    pub number: usize,
    pub atom_count: u16,
    pub word_count: u16,
    pub data: Vec<u8>,
}

impl Page {
    pub fn lex(&self) -> Vec<Token> {
        let mut c = binrw::io::Cursor::new(&self.data);
        let mut tokens = Vec::new();
        let mut unknown_buf = Vec::new();

        loop {
            match Token::read(&mut c) {
                Ok(t) => {
                    if !unknown_buf.is_empty() {
                        let decoded = String::from_utf8_lossy(&unknown_buf).to_string();
                        let unk = Token::Unknown {
                            raw: std::mem::replace(&mut unknown_buf, Vec::new()),
                            decoded,
                        };
                        tokens.push(unk);
                    }

                    tokens.push(t)
                }
                Err(e) if e.is_eof() => return tokens,
                Err(_) => {
                    let op = c.read_le::<u8>().unwrap();
                    unknown_buf.push(op);
                }
            }
        }
    }
}

#[binrw::binread]
#[derive(Debug)]
#[br(little)]
struct DkaBlock {
    #[br(temp, map = |x: u32| x + 1)]
    len: u32,

    #[br(count = len)]
    block: Vec<i32>,
}
