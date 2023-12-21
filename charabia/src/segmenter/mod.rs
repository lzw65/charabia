use std::borrow::Cow;
use std::collections::HashMap;

use aho_corasick::{AhoCorasick, FindIter, MatchKind};
pub use arabic::ArabicSegmenter;
#[cfg(feature = "chinese")]
pub use chinese::ChineseSegmenter;
use either::Either;
#[cfg(feature = "japanese")]
pub use japanese::JapaneseSegmenter;
#[cfg(feature = "korean")]
pub use korean::KoreanSegmenter;
pub use latin::LatinSegmenter;
use once_cell::sync::Lazy;
use slice_group_by::StrGroupBy;
#[cfg(feature = "thai")]
pub use thai::ThaiSegmenter;

use crate::detection::{Detect, Language, Script, StrDetection};
use crate::separators::DEFAULT_SEPARATORS;
use crate::token::Token;

mod arabic;
#[cfg(feature = "chinese")]
mod chinese;
#[cfg(feature = "japanese")]
mod japanese;
#[cfg(feature = "korean")]
mod korean;
mod latin;
#[cfg(feature = "thai")]
mod thai;
mod utils;

/// List of used [`Segmenter`]s linked to their corresponding [`Script`] and [`Language`].
///
/// This list is used after `Script` and `Language` detection to pick the specialized [`Segmenter`].
/// If no segmenter corresponds to the `Language`,
/// then the segmenter corresponding to the `Script` is picked.
/// If no segmenter corresponds to both `Script` and `Language`,
/// then the [`DEFAULT_SEGMENTER`] is picked.
///
/// A segmenter assigned to `Language::Other` is considered as the default `Segmenter` for any `Language` that uses the assigned `Script`.
/// For example, [`LatinSegmenter`] is assigned to `(Script::Latin, Language::Other)`,
/// meaning that `LatinSegmenter` is the default `Segmenter` for any `Language` that uses `Latin` `Script`.
pub static SEGMENTERS: Lazy<HashMap<(Script, Language), Box<dyn Segmenter>>> = Lazy::new(|| {
    vec![
        // latin segmenter
        ((Script::Latin, Language::Other), Box::new(LatinSegmenter) as Box<dyn Segmenter>),
        // chinese segmenter
        #[cfg(feature = "chinese")]
        ((Script::Cj, Language::Cmn), Box::new(ChineseSegmenter) as Box<dyn Segmenter>),
        // japanese segmenter
        #[cfg(feature = "japanese")]
        ((Script::Cj, Language::Jpn), Box::new(JapaneseSegmenter) as Box<dyn Segmenter>),
        // korean segmenter
        #[cfg(feature = "korean")]
        ((Script::Hangul, Language::Kor), Box::new(KoreanSegmenter) as Box<dyn Segmenter>),
        // thai segmenter
        #[cfg(feature = "thai")]
        ((Script::Thai, Language::Tha), Box::new(ThaiSegmenter) as Box<dyn Segmenter>),
        // arabic segmenter
        ((Script::Arabic, Language::Ara), Box::new(ArabicSegmenter) as Box<dyn Segmenter>),
    ]
    .into_iter()
    .collect()
});

/// Picked [`Segmenter`] when no segmenter is specialized to the detected [`Script`].
pub static DEFAULT_SEGMENTER: Lazy<Box<dyn Segmenter>> = Lazy::new(|| Box::new(LatinSegmenter));

pub static DEFAULT_SEPARATOR_AHO: Lazy<AhoCorasick> = Lazy::new(|| {
    AhoCorasick::builder().match_kind(MatchKind::LeftmostLongest).build(DEFAULT_SEPARATORS).unwrap()
});

/// Iterator over segmented [`Token`]s.
pub struct SegmentedTokenIter<'o, 'tb> {
    inner: SegmentedStrIter<'o, 'tb>,
    char_index: usize,
    byte_index: usize,
}

impl<'o> Iterator for SegmentedTokenIter<'o, '_> {
    type Item = Token<'o>;

    fn next(&mut self) -> Option<Self::Item> {
        let token_item = self.inner.next()?;
        match  token_item {
            TokenItem::Simple(lemma) => {
                let char_start = self.char_index;
                let byte_start = self.byte_index;
        
                self.char_index += lemma.chars().count();
                self.byte_index += lemma.len();
        
                Some(Token {
                    lemma: Cow::Borrowed(lemma),
                    script: self.inner.script,
                    language: self.inner.language,
                    char_start,
                    char_end: self.char_index,
                    byte_start,
                    byte_end: self.byte_index,
                    ..Default::default()
                })
            },
            TokenItem::WithPosition { text, char_start, char_end, byte_start, byte_end, is_last_token } => {
                let token = Some(Token {
                    lemma: Cow::Borrowed(text),
                    script: self.inner.script,
                    language: self.inner.language,
                    char_start: self.char_index + char_start,
                    char_end: self.char_index + char_end,
                    byte_start: self.byte_index + byte_start,
                    byte_end: self.byte_index + byte_end,
                    ..Default::default()
                });
                if is_last_token {
                    self.char_index += char_end;
                    self.byte_index += byte_end;
                }
                token
            }
        }
    }
}

impl<'o, 'tb> From<SegmentedStrIter<'o, 'tb>> for SegmentedTokenIter<'o, 'tb> {
    fn from(segmented_str_iter: SegmentedStrIter<'o, 'tb>) -> Self {
        Self { inner: segmented_str_iter, char_index: 0, byte_index: 0 }
    }
}

pub struct SegmentedStrIter<'o, 'tb> {
    inner: Box<dyn Iterator<Item = &'o str> + 'o>,
    current: Box<dyn Iterator<Item = TokenItem<'o> > + 'o>,
    aho_iter: Option<AhoSegmentedStrIter<'o, 'tb>>,
    segmenter: &'static dyn Segmenter,
    options: &'tb SegmenterOption<'tb>,
    script: Script,
    language: Option<Language>,
}

impl<'o, 'tb> SegmentedStrIter<'o, 'tb> {
    pub fn new(original: &'o str, options: &'tb SegmenterOption<'tb>) -> Self {
        let mut current_script = Script::Other;
        let inner = original.linear_group_by_key(move |c| {
            let script = Script::from(c);
            if script != Script::Other && script != current_script {
                current_script = script
            }
            current_script
        });

        Self {
            inner: Box::new(inner),
            current: Box::new(None.into_iter()),
            aho_iter: None,
            segmenter: &*DEFAULT_SEGMENTER,
            options,
            script: Script::Other,
            language: None,
        }
    }
}

impl<'o, 'tb> Iterator for SegmentedStrIter<'o, 'tb> {
    type Item = TokenItem<'o>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.current.next() {
            Some(s) => Some(s),
            None => match self.aho_iter.as_mut().and_then(|aho_iter| aho_iter.next()) {
                Some((s, MatchType::Match)) => Some(TokenItem::Simple(s)),
                Some((s, MatchType::Interleave)) => {
                    self.current = self.segmenter.segment_str(s);

                    self.next()
                }
                None => {
                    let text = self.inner.next()?;
                    let mut detector = text.detect(self.options.allow_list);
                    self.segmenter = segmenter(&mut detector);
                    self.script = detector.script();
                    self.language = detector.language;
                    self.aho_iter = Some(AhoSegmentedStrIter::new(
                        text,
                        self.options.aho.as_ref().unwrap_or(&DEFAULT_SEPARATOR_AHO),
                    ));

                    self.next()
                }
            },
        }
    }
}

struct AhoSegmentedStrIter<'o, 'aho> {
    aho_iter: FindIter<'aho, 'o>,
    prev: Either<usize, aho_corasick::Match>,
    text: &'o str,
}

impl<'o, 'aho> AhoSegmentedStrIter<'o, 'aho> {
    fn new(text: &'o str, aho: &'aho AhoCorasick) -> Self {
        Self { aho_iter: aho.find_iter(text), prev: Either::Left(0), text }
    }
}

impl<'o, 'aho> Iterator for AhoSegmentedStrIter<'o, 'aho> {
    type Item = (&'o str, MatchType);

    fn next(&mut self) -> Option<Self::Item> {
        let mut match_type = MatchType::Interleave;
        let (start, end) = match self.prev {
            Either::Left(left) => match self.aho_iter.next() {
                Some(m) => {
                    let range = (left, m.start());
                    self.prev = Either::Right(m);
                    range
                }
                None => {
                    self.prev = Either::Left(self.text.len());
                    (left, self.text.len())
                }
            },
            Either::Right(m) => {
                self.prev = Either::Left(m.end());
                match_type = MatchType::Match;
                (m.start(), m.end())
            }
        };

        if start < end {
            Some((&self.text[start..end], match_type))
        } else if end < self.text.len() {
            self.next()
        } else {
            None
        }
    }
}

enum MatchType {
    Interleave,
    Match,
}

/// Try to Detect Language and Script and return the corresponding segmenter,
/// if no Language is detected or no segmenter corresponds to the Language
/// the function try to get a segmenter corresponding to the script;
/// if no Script is detected or no segmenter corresponds to the Script,
/// the function try to get the default segmenter in the map;
/// if no default segmenter exists in the map return the library DEFAULT_SEGMENTER.
fn segmenter<'b>(detector: &mut StrDetection) -> &'b dyn Segmenter {
    let detected_script = detector.script();
    let mut filtered_segmenters =
        SEGMENTERS.iter().filter(|((script, _), _)| *script == detected_script);
    match (filtered_segmenters.next(), filtered_segmenters.next()) {
        // no specialized segmenter found for this script,
        // choose the default one.
        (None, None) => &*DEFAULT_SEGMENTER,
        // Only one specialized segmenter found,
        // we don't need to detect the Language.
        (Some((_, segmenter)), None) => segmenter,
        // several segmenters found,
        // we have to detect the language to get the good one.
        _ => {
            let detected_language = detector.language();
            SEGMENTERS
                .get(&(detected_script, detected_language))
                .or_else(|| SEGMENTERS.get(&(detected_script, Language::Other)))
                .unwrap_or(&DEFAULT_SEGMENTER)
        }
    }
}

/// Structure for providing options to a normalizer.
#[derive(Clone, Default)]
pub struct SegmenterOption<'tb> {
    pub aho: Option<AhoCorasick>,
    pub allow_list: Option<&'tb HashMap<Script, Vec<Language>>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenItem<'a> {
    Simple(&'a str),
    WithPosition {
        text: &'a str,
        char_start: usize,
        char_end: usize,
        byte_start: usize,
        byte_end: usize,
        is_last_token: bool,
    },
}
/// Trait defining a segmenter.
///
/// A segmenter should be at least a script specialized segmenter.
pub trait Segmenter: Sync + Send {
    /// Segments the provided text creating an Iterator over `&str`.
    fn segment_str<'o>(&self, s: &'o str) -> Box<dyn Iterator<Item =TokenItem<'o> > + 'o>;
}

impl Segmenter for Box<dyn Segmenter> {
    fn segment_str<'o>(&self, s: &'o str) -> Box<dyn Iterator<Item = TokenItem<'o>> + 'o> {
        (**self).segment_str(s)
    }
}

/// Trait defining methods to segment a text.
pub trait Segment<'o> {
    /// Segments the provided text creating an Iterator over Tokens.
    /// Created Tokens are not normalized nether classified,
    /// otherwise, better use the [`tokenize`] method.
    ///
    /// [`tokenize`]: crate::tokenizer::Tokenize#tymethod.tokenize
    ///
    /// # Example
    ///
    /// ```
    /// use charabia::{Token, TokenKind, Segment};
    ///
    /// let orig = "The quick (\"brown\") fox can't jump 32.3 feet, right? Brr, it's 29.3°F!";
    ///
    /// let mut tokens = orig.segment();
    ///
    /// let Token { lemma, kind, .. } = tokens.next().unwrap();
    /// // the token isn't normalized.
    /// assert_eq!(lemma, "The");
    /// // the token isn't classified and defaultly set to Unknown.
    /// assert_eq!(kind, TokenKind::Unknown);
    ///
    /// let Token { lemma, kind, .. } = tokens.next().unwrap();
    /// assert_eq!(lemma, " ");
    /// assert_eq!(kind, TokenKind::Unknown);
    ///
    /// let Token { lemma, kind, .. } = tokens.next().unwrap();
    /// assert_eq!(lemma, "quick");
    /// assert_eq!(kind, TokenKind::Unknown);
    /// ```
    fn segment(&self) -> SegmentedTokenIter<'o, 'o> {
        self.segment_str().into()
    }

    /// Segments the provided text creating an Iterator over Tokens where you can specify an allowed list of languages to be used with a script.
    fn segment_with_option<'tb>(
        &self,
        options: &'tb SegmenterOption<'tb>,
    ) -> SegmentedTokenIter<'o, 'tb> {
        self.segment_str_with_option(options).into()
    }

    /// Segments the provided text creating an Iterator over `&str`.
    ///
    /// # Example
    ///
    /// ```
    /// use charabia::Segment;
    ///
    /// let orig = "The quick (\"brown\") fox can't jump 32.3 feet, right? Brr, it's 29.3°F!";
    ///
    /// let mut segments = orig.segment_str();
    ///
    /// assert_eq!(segments.next(), Some("The"));
    /// assert_eq!(segments.next(), Some(" "));
    /// assert_eq!(segments.next(), Some("quick"));
    /// ```
    fn segment_str(&self) -> SegmentedStrIter<'o, 'o> {
        self.segment_str_with_option(&SegmenterOption { aho: None, allow_list: None })
    }

    /// Segments the provided text creating an Iterator over `&str` where you can specify an allowed list of languages to be used with a script.
    ///
    fn segment_str_with_option<'tb>(
        &self,
        options: &'tb SegmenterOption<'tb>,
    ) -> SegmentedStrIter<'o, 'tb>;
}

impl<'o> Segment<'o> for &'o str {
    fn segment_str_with_option<'tb>(
        &self,
        options: &'tb SegmenterOption<'tb>,
    ) -> SegmentedStrIter<'o, 'tb> {
        SegmentedStrIter::new(self, options)
    }
}

#[cfg(test)]
mod test {
    macro_rules! test_segmenter {
    ($segmenter:expr, $text:expr, $segmented:expr, $tokenized:expr, $script:expr, $language:expr) => {
            use crate::{Token, Language, Script};
            use crate::segmenter::{TokenItem, Segment, AhoSegmentedStrIter, MatchType, DEFAULT_SEPARATOR_AHO};
            use crate::tokenizer::Tokenize;
            use super::*;

            #[test]
            fn segmenter_segment_str() {

                let segmented_text: Vec<_> = AhoSegmentedStrIter::new($text, &DEFAULT_SEPARATOR_AHO).flat_map(|m| match m {
                    (text, MatchType::Match) => Box::new(Some(text).into_iter()),
                    // (text, MatchType::Interleave) => $segmenter.segment_str(text),
                    (text, MatchType::Interleave) => {
                        let result = $segmenter.segment_str(text);
                        let s_vector:Vec<_> = result.map(|token_item| match token_item {
                                TokenItem::Simple(s) => Some(s),
                                TokenItem::WithPosition { text, .. } => Some(text),
                            }).collect();
                        Box::new(s_vector.into_iter())
                    }
    
                }).collect();
                assert_eq!(&segmented_text[..], $segmented, r#"
Segmenter {} didn't segment the text as expected.

help: the `segmented` text provided to `test_segmenter!` does not corresponds to the output of the tested segmenter, it's probably due to a bug in the segmenter or a mistake in the provided segmented text.
"#, stringify!($segmenter));
            }

            #[test]
            fn text_lang_script_assignment() {
                let Token {script, language, ..} = $text.segment().next().unwrap();
                assert_eq!((script, language.unwrap_or($language)), ($script, $language), r#"
Provided text is not detected as the expected Script or Language to be segmented by {}.

help: The tokenizer Script/Language detector detected the wrong Script/Language for the `segmented` text, the provided text will probably be segmented by an other segmenter.
Check if the expected Script/Language corresponds to the detected Script/Language.
"#, stringify!($segmenter));
            }

            #[test]
            fn segment() {
                let segmented_text: Vec<_> = $text.segment_str().collect();
                assert_eq!(&segmented_text[..], $segmented, r#"
Segmenter chosen by global segment() function, didn't segment the text as expected.

help: The selected segmenter is probably the wrong one.
Check if the tested segmenter is assigned to the good Script/Language in `SEGMENTERS` global in `charabia/src/segmenter/mod.rs`.
"#);
            }

            #[test]
            fn tokenize() {
                let tokens: Vec<_> = $text.tokenize().collect();
                let tokenized_text: Vec<_> = tokens.iter().map(|t| t.lemma()).collect();

                assert_eq!(&tokenized_text[..], $tokenized, r#"
Global tokenize() function didn't tokenize the text as expected.

help: The normalized version of the segmented text is probably wrong, the used normalizers make unexpeted changes to the provided text.
Make sure that normalized text is valid or change the trigger condition of the noisy normalizers by updating `should_normalize`.
"#);
            }
        }
    }
    pub(crate) use test_segmenter;
}
