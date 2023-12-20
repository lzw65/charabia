#[cfg(feature = "latin-camelcase")]
mod camel_case;

use crate::segmenter:: {Segmenter, TokenItem};

/// Latin specialized [`Segmenter`].
///
pub struct LatinSegmenter;

impl Segmenter for LatinSegmenter {
    #[cfg(not(feature = "latin-camelcase"))]
    fn segment_str<'o>(&self, s: &'o str) -> Box<dyn Iterator<Item =TokenItem<'o> > + 'o> {
        let token_items: Vec<TokenItem> = Some(s).into_iter()
            .map(|lemma| TokenItem::Simple(lemma))
            .collect();
        Box::new(token_items.into_iter())
    }

    #[cfg(feature = "latin-camelcase")]
    fn segment_str<'o>(&self, s: &'o str) -> Box<dyn Iterator<Item = TokenItem<'o> > + 'o> {
        let lemmas = camel_case::split_camel_case_bounds(s);
        let token_items: Vec<TokenItem> = lemmas
            .into_iter()
            .map(|lemma| TokenItem::Simple(lemma))
            .collect();
        Box::new(token_items.into_iter())
    }
}

#[cfg(test)]
mod test {
    use crate::segmenter::test::test_segmenter;

    const TEXT: &str =
        "The quick (\"brown\") fox can’t jump 32.3 feet, right? Brr, it's 29.3°F! camelCase kebab-case snake_case";
    const SEGMENTED: &[&str] = &[
        "The", " ", "quick", " ", "(", "\"", "brown", "\"", ")", " ", "fox", " ", "can", "’", "t",
        " ", "jump", " ", "32", ".", "3", " ", "feet", ", ", "right", "?", " ", "Brr", ", ", "it",
        "'", "s", " ", "29", ".", "3°F", "!", " ", "camel", "Case", " ", "kebab", "-", "case", " ",
        "snake", "_", "case",
    ];
    const TOKENIZED: &[&str] = &[
        "the", " ", "quick", " ", "(", "\"", "brown", "\"", ")", " ", "fox", " ", "can", "'", "t",
        " ", "jump", " ", "32", ".", "3", " ", "feet", ", ", "right", "?", " ", "brr", ", ", "it",
        "'", "s", " ", "29", ".", "3°f", "!", " ", "camel", "case", " ", "kebab", "-", "case", " ",
        "snake", "_", "case",
    ];

    test_segmenter!(LatinSegmenter, TEXT, SEGMENTED, TOKENIZED, Script::Latin, Language::Other);
}
