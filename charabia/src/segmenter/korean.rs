use lindera_core::mode::{Mode, Penalty};
use lindera_dictionary::{DictionaryConfig, DictionaryKind};
use lindera_tokenizer::tokenizer::{Tokenizer, TokenizerConfig};
use once_cell::sync::Lazy;

use crate::segmenter::{Segmenter, TokenItem};

/// Korean specialized [`Segmenter`].
///
/// This Segmenter uses lindera internally to segment the provided text.
pub struct KoreanSegmenter;

static LINDERA: Lazy<Tokenizer> = Lazy::new(|| {
    let config = TokenizerConfig {
        dictionary: DictionaryConfig { kind: Some(DictionaryKind::KoDic), path: None },
        mode: Mode::Decompose(Penalty::default()),
        ..TokenizerConfig::default()
    };
    Tokenizer::from_config(config).unwrap()
});

impl Segmenter for KoreanSegmenter {
    fn segment_str<'o>(&self, to_segment: &'o str) -> Box<dyn Iterator<Item =TokenItem<'o> > + 'o> {
        let segment_iterator = LINDERA.tokenize(to_segment).unwrap();
        let token_items: Vec<TokenItem> = segment_iterator
        .iter()
        .map(|token| TokenItem::Simple(token.text))
        .collect();
        Box::new(token_items.into_iter())
    }
}

#[cfg(test)]
mod test {
    use crate::segmenter::test::test_segmenter;

    const TEXT: &str = "한국어의형태해석을실시할수있습니다.";

    const SEGMENTED: &[&str] =
        &["한국어", "의", "형태", "해석", "을", "실시", "할", "수", "있", "습니다", "."];

    const TOKENIZED: &[&str] =
        &["한국어", "의", "형태", "해석", "을", "실시", "할", "수", "있", "습니다", "."];

    // Macro that run several tests on the Segmenter.
    test_segmenter!(KoreanSegmenter, TEXT, SEGMENTED, TOKENIZED, Script::Hangul, Language::Kor);
}
