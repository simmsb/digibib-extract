use std::fmt::Write;

use crate::{decoding, text::Page, token::Token};

pub struct State {
    italic: bool,
    bold: bool,
    font_size: u8,
    font_idx: u8,
    word_incomplete: bool,
    had_carriage_return: bool,
    add_hyphen_at_eol: bool,
    add_hyphen_at_eol_separating_ck: bool,
    add_invisible_hyphen: bool,
}

impl State {
    fn new() -> Self {
        Self {
            italic: false,
            bold: false,
            font_size: 10,
            font_idx: 0,
            word_incomplete: false,
            had_carriage_return: false,
            add_hyphen_at_eol: false,
            add_hyphen_at_eol_separating_ck: false,
            add_invisible_hyphen: false,
        }
    }

    fn reset_hyphens(&mut self) {
        self.add_hyphen_at_eol = false;
        self.add_hyphen_at_eol_separating_ck = false;
        self.add_invisible_hyphen = false;
    }

    fn hyphen(&self) -> bool {
        self.add_hyphen_at_eol || self.add_hyphen_at_eol_separating_ck || self.add_invisible_hyphen
    }
}

pub fn write_page(page: &Page, lexed: &[Token], mut output: impl Write) -> eyre::Result<()> {
    let mut state = State::new();

    for t in lexed {
        match t {
            Token::Blanks(number) => {
                for _ in 0..*number {
                    write!(output, " ")?;
                }
            }
            Token::Word { space_at_end, data } => {
                let s = decoding::decode_string(data, state.font_idx);

                if state.word_incomplete {
                    state.word_incomplete = false;
                } else {
                    if s.len() > 0 {
                        let mut closers = 0;
                        if state.font_size != 10 {
                            write!(output, "#text(size: {}em)[", state.font_size as f32 / 10.0)?;
                            closers += 1;
                        }
                        if state.italic {
                            write!(output, "#emph[")?;
                            closers += 1;
                        }
                        if state.bold {
                            write!(output, "#strong[")?;
                            closers += 1;
                        }

                        write!(output, "{}", s)?;

                        for _ in 0..closers {
                            write!(output, "]")?;
                        }
                    }
                }

                state.reset_hyphens();
                if *space_at_end || s.chars().next_back().map_or(false, |c| !c.is_alphanumeric()) {
                    write!(output, " ")?;
                }

            }
            Token::HardCarriageReturn => {
                state.had_carriage_return = true;
                writeln!(output, "\\")?;
            }
            Token::EndOfPage => { break; }
            Token::ItalicsOn => { state.italic = true; }
            Token::ItalicsOff => { state.italic = false; }
            Token::BoldOn => { state.bold = true; }
            Token::BoldOff => { state.bold = false; }
            Token::FontPreset(n) => {
                match n {
                    0 => {
                        state.font_size = 10;
                        state.bold = false;
                        state.italic = false;
                    }
                    1 => {
                        state.font_size = 13;
                    }
                    2 => {
                        state.font_size = 12;
                    }
                    3 => {
                        state.font_size = 11;
                    }
                    4 => {
                        state.font_size = 10;
                        state.bold = true;
                    }
                    5 => {
                        state.font_size = 10;
                    }
                    6 => {
                        state.font_size = 10;
                        state.italic = true;
                    }
                    _ => {}
                }
            }
            Token::Ly => {}
            Token::Image { width, name } => {}
            Token::ImageLink(_) => {}
            Token::EndLink => {}
            Token::Font(_) => {}
            Token::FileName(_) => {}
            Token::Concordance(_) => {}
            Token::NodeNumber(_) => {}
            Token::SuperScriptOn => {}
            Token::SuperScriptOff => {}
            Token::Sigil(_) => {}
            Token::Header => {}
            Token::HypenAtEol => {}
            Token::UnderlineOn => {}
            Token::UnderlineOff => {}
            Token::GreekOn => {}
            Token::GreekOff => {}
            Token::OneBlank => {}
            Token::VerticalLineOn => {}
            Token::VerticalLineOff => {}
            Token::TD => {}
            Token::Null => {}
            Token::PageLink { page_number, name } => {}
            Token::IDStart => {}
            Token::IDEnd => {}
            Token::SubscriptOn => {}
            Token::SubscriptOff => {}
            Token::Color(_) => {}
            Token::InlineImage {
                width,
                height,
                name,
            } => {}
            Token::SearchWord(_) => {}
            Token::FontSize(_) => {}
            Token::Copyright(_) => {}
            Token::AutoLink(_) => {}
            Token::SoftCarriageReturn => {}
            Token::InvisibleHyphen => {}
            Token::LetterSpacingOn => {}
            Token::LetterSpacingOff => {}
            Token::HalfLineSpacing => {}
            Token::ListItemStart => {}
            Token::ListItemEnd => {}
            Token::UnorderedListStart => {}
            Token::UnorderedListEnd => {}
            Token::SetX(_) => {}
            Token::SV(_) => {}
            Token::SVLemmaBegin(_) => {}
            Token::SVLemmaStop => {}
            Token::CenteredOn => {}
            Token::CenteredOff => {}
            Token::AlignRightOn => {}
            Token::AlignRightOff => {}
            Token::EOn => {}
            Token::EOff => {}
            Token::BibIndex(_) => {}
            Token::NotFirstLine => {}
            Token::Thumb => {}
            Token::EndNew(_) => {}
            Token::UrlBegin(_) => {}
            Token::UrlEnd => {}
            Token::WordAnchor => {}
            Token::ThumbWWW => {}
            Token::S => {}
            Token::NoJustifyOn => {}
            Token::NoJustifyOff => {}
            Token::NextBlankFixed => {}
            Token::WordRest(_) => {}
            Token::WordIncomplete(_) => {}
            Token::HyphenCK => {}
            Token::HebrewOn => {}
            Token::HebrewOff => {}
            Token::NodeNumber2(_) => {}
            Token::StrikeThroughOn => {}
            Token::StrikeThroughOff => {}
            Token::SetY(_) => {}
            Token::Cor(_) => {}
            Token::EndCor => {}
            Token::DashedLine => {}
            Token::Unknown { raw, decoded } => {}
        }
    }

    Ok(())
}

pub fn count_delimeters(s: &str, hyphen: bool) -> usize {
    let v = if !hyphen && !s.chars().all(|c| !c.is_alphanumeric()) {
        1
    } else {
        0
    };

    let mut it = s.chars();

    while let Some(c) = it.next() {
        if c.is_alphanumeric() {
            break;
        }
    }

    while let Some(c) = it.next_back() {
        if c.is_alphanumeric() {
            break;
        }
    }

    v + it.filter(|c| !c.is_alphanumeric()).count()
}
