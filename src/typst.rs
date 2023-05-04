use std::{
    collections::{HashMap, HashSet},
    fmt::Write,
    sync::Arc,
};

use once_cell::sync::{Lazy, OnceCell};
use regex::Regex;

use crate::{decoding, text::Page, toc::TocItem, token::Token};

pub struct State<W> {
    writer: W,
    font_idx: u8,
    word_incomplete: bool,
    had_carriage_return: bool,
    add_hyphen_at_eol: bool,
    add_hyphen_at_eol_separating_ck: bool,
    add_invisible_hyphen: bool,
    file_name: Option<String>,
    concordance: Option<u16>,
    node_number: Option<u16>,
    sigil: Option<String>,
    current_functions: Vec<(&'static str, String)>,
}

impl<W: Write> State<W> {
    fn new(writer: W) -> Self {
        Self {
            writer,
            font_idx: 0,
            word_incomplete: false,
            had_carriage_return: false,
            add_hyphen_at_eol: false,
            add_hyphen_at_eol_separating_ck: false,
            add_invisible_hyphen: false,
            file_name: None,
            concordance: None,
            node_number: None,
            sigil: None,
            current_functions: Vec::new(),
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

    fn push_state(&mut self, key: &'static str, val: impl AsRef<str>) -> eyre::Result<()> {
        self.pop_state(key)?;
        self.current_functions.push((key, val.as_ref().to_owned()));

        write!(self, "#{}[", val.as_ref())?;

        Ok(())
    }

    fn pop_all_states(&mut self) -> eyre::Result<()> {
        for _ in 0..self.current_functions.len() {
            write!(self, "]")?;
        }

        self.current_functions.clear();

        Ok(())
    }

    fn pop_state(&mut self, key: &str) -> eyre::Result<()> {
        let idx = self
            .current_functions
            .iter()
            .enumerate()
            .find(|(_, (k, _))| *k == key);

        if let Some((idx, (_, _))) = idx {
            let after_this = self.current_functions.len() - idx - 1;

            // if there are states pushed after the state we're about to remove,
            // we need to temporarily close those before we can close this state

            write!(self, "]")?;

            for _ in 0..after_this {
                write!(self, "]")?;
            }

            for (_, v) in &self.current_functions[(idx + 1)..] {
                write!(self.writer, "#{}[", v)?;
            }

            self.current_functions.remove(idx);
        }

        Ok(())
    }
}

impl<W: Write> Write for State<W> {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        self.writer.write_str(s)
    }
}

pub fn write_page(
    tocitem: &TocItem,
    page_number: usize,
    lexed: &[Token],
    output: impl Write,
) -> eyre::Result<()> {
    static ESCAPER: Lazy<Regex> = Lazy::new(|| Regex::new(r"[#()\[\]*=_`<>/$]").unwrap());

    let mut state = State::new(output);

    writeln!(
        state,
        "#align(center)[#heading(level: {}, numbering: \"1.a.\")[{}] <page{}>]",
        tocitem.level,
        ESCAPER.replace_all(&tocitem.title, "\\$0"),
        page_number
    )?;

    for t in lexed {
        match t {
            Token::Blanks(number) => {
                for _ in 0..*number {
                    write!(state, " ")?;
                }
            }
            Token::Word { space_at_end, data } => {
                let s = decoding::decode_string(data, state.font_idx);

                if state.word_incomplete {
                    state.word_incomplete = false;
                } else {
                    if s.len() > 0 {
                        let s = if !state.hyphen() {
                            s.trim_end().trim_end_matches('-')
                        } else {
                            &s
                        };

                        write!(state, "{}", ESCAPER.replace_all(s, "\\$0"))?;
                    }
                }

                state.reset_hyphens();
                if *space_at_end
                    || s.chars()
                        .next_back()
                        .map_or(false, |c| !c.is_alphanumeric())
                {
                    write!(state, " ")?;
                }
            }
            Token::HardCarriageReturn => {
                state.had_carriage_return = true;
                writeln!(state, "\\")?;
            }
            Token::EndOfPage => {
                break;
            }
            Token::ItalicsOn => {
                state.push_state("emph", "emph")?;
            }
            Token::ItalicsOff => {
                state.pop_state("emph")?;
            }
            Token::BoldOn => {
                state.push_state("strong", "strong")?;
            }
            Token::BoldOff => {
                state.pop_state("strong")?;
            }
            Token::FontPreset(n) => match n {
                0 => {
                    state.pop_state("color")?;
                    state.pop_state("strong")?;
                    state.pop_state("italic")?;
                }
                1 => {
                    state.push_state("size", format!("text(size: {}em)", 1.33))?;
                }
                2 => {
                    state.push_state("size", format!("text(size: {}em)", 1.22))?;
                }
                3 => {
                    state.push_state("size", format!("text(size: {}em)", 1.11))?;
                }
                4 => {
                    state.push_state("size", format!("text(size: {}em)", 1.0))?;
                    state.push_state("strong", "strong")?;
                }
                5 => {
                    state.push_state("size", format!("text(size: {}em)", 1.0))?;
                }
                6 => {
                    state.push_state("size", format!("text(size: {}em)", 1.0))?;
                    state.push_state("emph", "emph")?;
                }
                _ => {}
            },
            Token::Ly => {
                // ???
            }
            Token::Image { width, name } => {}
            Token::ImageLink(_) => {}
            Token::EndLink => {}
            Token::Font(n) => {
                state.font_idx = *n;
            }
            Token::FileName(s) => {
                state.file_name = Some(s.data.to_owned());
            }
            Token::Concordance(n) => {
                state.concordance = Some(*n);
            }
            Token::NodeNumber(n) => {
                state.node_number = Some(*n);
            }
            Token::SuperScriptOn => {
                state.push_state("super", "super")?;
            }
            Token::SuperScriptOff => {
                state.pop_state("super")?;
            }
            Token::Sigil(s) => {
                state.sigil = Some(s.data.clone());
            }
            Token::Header => {}
            Token::HypenAtEol => {
                state.add_invisible_hyphen = true;
            }
            Token::UnderlineOn => {
                state.push_state("underline", "underline")?;
            }
            Token::UnderlineOff => {
                state.pop_state("underline")?;
            }
            Token::GreekOn => {}
            Token::GreekOff => {}
            Token::OneBlank => {
                write!(state, " ")?;
            }
            Token::VerticalLineOn => {
                // not used
            }
            Token::VerticalLineOff => {}
            Token::TD => {}
            Token::Null => {}
            Token::PageLink { page_number, name } => {
                if *page_number != 0 {
                    write!(state, " @page{} ", page_number)?;
                } else {
                    // TODO image link
                }
            }
            Token::IDStart(_) => {}
            Token::IDEnd(_) => {}
            Token::SubscriptOn => {
                state.push_state("sub", "sub")?;
            }
            Token::SubscriptOff => {
                state.pop_state("sub")?;
            }
            Token::Color(colour) => {
                if *colour == 1 {
                    state.push_state("colour", "text(fill: gray)")?;
                } else {
                    state.pop_state("colour")?;
                }
            }
            Token::InlineImage {
                width,
                height,
                name,
            } => {}
            Token::SearchWord(_) => {}
            Token::FontSize(size) => {
                state.push_state("size", format!("text(size: {:02}em)", *size as f32 / 100.0))?;
            }
            Token::Copyright(_) => {}
            Token::AutoLink(page) => {
                write!(state, "@page{}", page)?;
            }
            Token::SoftCarriageReturn => {
                if !state.hyphen() {
                    write!(state, " ")?;
                }
            }
            Token::InvisibleHyphen => {
                state.add_invisible_hyphen = true;
            }
            Token::LetterSpacingOn => {
                state.push_state("tracking", "text(tracking: 1.5pt)")?;
            }
            Token::LetterSpacingOff => {
                state.pop_state("tracking")?;
            }
            Token::HalfLineSpacing => {
                write!(state, "\n")?;
            }
            Token::ListItemStart => {
                write!(state, "[")?;
            }
            Token::ListItemEnd => {
                write!(state, "]")?;
            }
            Token::UnorderedListStart => {
                write!(state, "#list[")?;
            }
            Token::UnorderedListEnd => {
                write!(state, "]")?;
            }
            Token::SetX(indent) => {
                state.push_state("padding", format!("pad(x: {}pt)", *indent as f32 / 100.0))?;
            }
            Token::SV(_) => {}
            Token::SVLemmaBegin(_) => {}
            Token::SVLemmaStop => {}
            Token::CenteredOn => {
                state.push_state("align-center", "align(center)")?;
            }
            Token::CenteredOff => {
                state.pop_state("align-center")?;
            }
            Token::AlignRightOn => {
                state.push_state("align-right", "align(right)")?;
            }
            Token::AlignRightOff => {
                state.pop_state("align-right")?;
            }
            Token::EOn => {}
            Token::EOff => {}
            Token::BibIndex(_) => {}
            Token::NotFirstLine => {}
            Token::Thumb => {}
            Token::EndNew(_) => {}
            Token::UrlBegin(url) => {
                state.push_state("link", format!("link(\"{}\")", url.data))?;
            }
            Token::UrlEnd => {
                state.pop_state("link")?;
            }
            Token::WordAnchor => {}
            Token::ThumbWWW => {}
            Token::S => {}
            Token::NoJustifyOn => {
                state.push_state("nojustify", "par(justify: false)")?;
            }
            Token::NoJustifyOff => {
                state.pop_state("nojustify")?;
            }
            Token::NextBlankFixed => {}
            Token::WordRest { space_at_end, data } => {
                write!(state, "{}", data)?;

                if *space_at_end {
                    write!(state, " ")?;
                }
            }
            Token::WordIncomplete(word) => {
                write!(state, "{}", word.data)?;
                state.word_incomplete = true;
            }
            Token::HyphenCK => {
                state.add_hyphen_at_eol_separating_ck = true;
            }
            Token::HebrewOn => {}
            Token::HebrewOff => {}
            Token::NodeNumber2(_) => {}
            Token::StrikeThroughOn => {
                state.push_state("strike", "strikethrough")?;
            }
            Token::StrikeThroughOff => {
                state.pop_state("strike")?;
            }
            Token::SetY(_) => {}
            Token::Cor(_) => {}
            Token::EndCor => {}
            Token::DashedLine => {}
            Token::Unknown { raw, decoded } => {}
        }
    }

    state.pop_all_states()?;

    writeln!(state, "\n#pagebreak(weak: true)")?;

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
