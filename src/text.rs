use binrw::{BinRead, BinReaderExt, VecArgs};
use std::{
    fmt::Debug,
    io::{BufRead, BufReader, Read},
    iter::Peekable,
};

use encoding_rs::WINDOWS_1252;
use encoding_rs_io::DecodeReaderBytesBuilder;

pub struct Pages {
    pub start: usize,
    pub pages: Vec<Page>,
}

impl Pages {
    pub fn load(
        mut text_dki: impl BinReaderExt,
        page_number: usize,
        count: usize,
    ) -> eyre::Result<Self> {
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

        let pages = (page_number..(page_number + count))
            .map(|i| Self::load_page(&mut text_dki, &page_table, i, has_magic))
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

#[binrw::binread]
#[br(little)]
pub struct Name {
    #[br(temp)]
    len: u8,

    #[br(count = len,  map = |buf: Vec<u8>| encoding_rs::WINDOWS_1252.decode(&buf).0.to_string() )]
    data: String,
}

impl Debug for Name {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Name").field(&self.data).finish()
    }
}

#[binrw::binread]
#[derive(Debug)]
#[br(little)]
pub enum Token {
    #[br(magic = 0u8)]
    Blanks(u8),
    #[br(magic = 1u8)]
    Word {
        #[br(temp)]
        len: u8,

        #[br(count = len & 0x7f)]
        data: Vec<u8>,
    },
    #[br(magic = 2u8)]
    HardCarriageReturn,
    #[br(magic = 3u8)]
    EndOfPage,
    #[br(magic = 4u8)]
    ItalicsOn,
    #[br(magic = 5u8)]
    ItalicsOff,
    #[br(magic = 6u8)]
    BoldOn,
    #[br(magic = 7u8)]
    BoldOff,
    #[br(magic = 8u8)]
    SetFont(u8),
    #[br(magic = 9u8)]
    Ly,
    #[br(magic = 10u8)]
    Image { width: u32, name: Name },
    #[br(magic = 11u8)]
    ImageLink(Name),
    #[br(magic = 12u8)]
    EndLink,
    #[br(magic = 13u8)]
    Font(u8),
    #[br(magic = 14u8)]
    FileName(Name),
    #[br(magic = 15u8)]
    Concordance(u16),
    #[br(magic = 16u8)]
    NodeNumber(u16),
    #[br(magic = 17u8)]
    SuperScriptOn,
    #[br(magic = 18u8)]
    SuperScriptOff,
    #[br(magic = 19u8)]
    Sigil(Name),
    #[br(magic = 20u8)]
    Header,
    #[br(magic = 21u8)]
    HypenAtEol,
    #[br(magic = 22u8)]
    UnderlineOn,
    #[br(magic = 23u8)]
    UnderlineOff,
    #[br(magic = 24u8)]
    GreekOn,
    #[br(magic = 25u8)]
    GreekOff,
    #[br(magic = 27u8)]
    OneBlank,
    #[br(magic = 28u8)]
    VerticalLineOn,
    #[br(magic = 29u8)]
    VerticalLineOff,
    #[br(magic = 30u8)]
    TD,
    #[br(magic = 31u8)]
    Null,
    #[br(magic = 32u8)]
    PageLink { page_number: u32, name: Name },
    #[br(magic = 129u8)]
    IDStart,
    #[br(magic = 130u8)]
    IDEnd,
    #[br(magic = 131u8)]
    SubscriptOn,
    #[br(magic = 132u8)]
    SubscriptOff,
    #[br(magic = 133u8)]
    Color(u8),
    #[br(magic = 134u8)]
    InlineImage { width: u16, height: u16, name: Name },
    #[br(magic = 135u8)]
    SearchWord(Name),
    #[br(magic = 136u8)]
    FontSize(u8),
    #[br(magic = 137u8)]
    Copyright(u8),
    #[br(magic = 138u8)]
    AutoLink(u32),
    #[br(magic = 139u8)]
    SoftCarriageReturn,
    #[br(magic = 140u8)]
    InvisibleHyphen,
    #[br(magic = 141u8)]
    LetterSpacingOn,
    #[br(magic = 142u8)]
    LetterSpacingOff,
    #[br(magic = 143u8)]
    HalfLineSpacing,
    #[br(magic = 144u8)]
    ListItemStart,
    #[br(magic = 145u8)]
    ListItemEnd,
    #[br(magic = 146u8)]
    UnorderedListStart,
    #[br(magic = 147u8)]
    UnorderedListEnd,
    #[br(magic = 148u8)]
    SetX(u16),
    #[br(magic = 149u8)]
    SV(u64),
    #[br(magic = 150u8)]
    SVLemmaBegin(Name),
    #[br(magic = 151u8)]
    SVLemmaStop,
    #[br(magic = 152u8)]
    CenteredOn,
    #[br(magic = 153u8)]
    CenteredOff,
    #[br(magic = 154u8)]
    AlignRightOn,
    #[br(magic = 155u8)]
    AlignRightOff,
    #[br(magic = 156u8)]
    EOn,
    #[br(magic = 157u8)]
    EOff,
    #[br(magic = 158u8)]
    BibIndex(u32),
    #[br(magic = 159u8)]
    NotFirstLine,
    #[br(magic = 160u8)]
    Thumb,
    #[br(magic = 161u8)]
    EndNew([u8; 3]),
    #[br(magic = 162u8)]
    UrlBegin(Name),
    #[br(magic = 163u8)]
    UrlEnd,
    #[br(magic = 164u8)]
    WordAnchor,
    #[br(magic = 165u8)]
    ThumbWWW,
    #[br(magic = 166u8)]
    S,
    #[br(magic = 167u8)]
    NoJustifyOn,
    #[br(magic = 168u8)]
    NoJustifyOff,
    #[br(magic = 169u8)]
    NextBlankFixed,
    #[br(magic = 170u8)]
    WordRest(Name),
    #[br(magic = 171u8)]
    WordIncomplete(Name),
    #[br(magic = 172u8)]
    HyphenCK,
    #[br(magic = 173u8)]
    HebrewOn,
    #[br(magic = 174u8)]
    HebrewOff,
    #[br(magic = 175u8)]
    NodeNumber2(u32),
    #[br(magic = 176u8)]
    StrikeThroughOn,
    #[br(magic = 177u8)]
    StrikeThroughOff,
    #[br(magic = 178u8)]
    SetY(u16),
    #[br(magic = 179u8)]
    Cor(u32),
    #[br(magic = 180u8)]
    EndCor,
    #[br(magic = 236u8)]
    DashedLine,
    #[br(magic = 255u8)]
    Unknown {
        #[br(count = 0)]
        raw: Vec<u8>,

        #[br(count = 0, map = |_: Vec<u8>| String::new())]
        decoded: String,
    },
}
