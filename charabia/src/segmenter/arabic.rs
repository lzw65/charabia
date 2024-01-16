use super::{Segmenter, TokenItem};

/// Arabic specialized [`Segmenter`].
///
/// Arabic text is segmented by word boundaries and by punctuation.
/// We need a workaround to segment the Arabic text that starts with `ال` (the) because it is not segmented by word boundaries.
/// One possible solution is to segment any word that starts with `ال` into two words. The `ال` and the rest of the word.
/// with this solution, we will have `الشجرة` (the tree) segmented into `ال` (the) and `شجرة` (tree). and if we search for `شجرة` (tree) or `الشجرة` (thetree) we will find results.
/// Some Arabic text starts with `ال` and not meant to be (the) like `البانيا` (Albania). In this case, we will have `ال` and `بانيا` segmented. and if we search for `البانيا` we will find results.

pub struct ArabicSegmenter;

// All specialized segmenters only need to implement the method `segment_str` of the `Segmenter` trait.
impl Segmenter for ArabicSegmenter {
    fn segment_str<'o>(&self, to_segment: &'o str, is_query: Option<bool>) -> Box<dyn Iterator<Item = TokenItem<'o> > + 'o> {
        // check if to_segment starts with 'ال', 'أل', 'إل', 'آل' or 'ٱل'
        if to_segment.len() > 2
            && (to_segment.starts_with("ال")
                || to_segment.starts_with("أل")
                || to_segment.starts_with("إل")
                || to_segment.starts_with("آل")
                || to_segment.starts_with("ٱل"))
        {
            let token_items: Vec<TokenItem> = vec![&to_segment[..4], &to_segment[4..]]
            .into_iter()
            .map(|s| TokenItem::Simple(s))
            .collect();
            Box::new(token_items.into_iter())
        } else {
            let token_items: Vec<TokenItem> = Some(to_segment).into_iter()
            .map(|s| TokenItem::Simple(s))
            .collect();
            Box::new(token_items.into_iter())
        }
    }
}

// Test the segmenter:
#[cfg(test)]
mod test {
    use crate::segmenter::test::test_segmenter;

    // Original version of the text.
    const TEXT: &str = "السلام عليكم، كيف حالكم؟ (أتمنى أن تكونوا بأفضل ٱلأحوال)";

    // Segmented version of the text.
    const SEGMENTED: &[&str] = &[
        "ال",
        "سلام",
        " ",
        "عليكم",
        "،",
        " ",
        "كيف",
        " ",
        "حالكم",
        "؟",
        " ",
        "(",
        "أتمنى",
        " ",
        "أن",
        " ",
        "تكونوا",
        " ",
        "بأفضل",
        " ",
        "ٱل",
        "أحوال",
        ")",
    ];

    // Segmented and normalized version of the text.
    const TOKENIZED: &[&str] = &[
        "ال",
        "سلام",
        " ",
        "عليكم",
        "،",
        " ",
        "كيف",
        " ",
        "حالكم",
        "؟",
        " ",
        "(",
        "اتمني",
        " ",
        "ان",
        " ",
        "تكونوا",
        " ",
        "بافضل",
        " ",
        "ال",
        "احوال",
        ")",
    ];

    // Macro that run several tests on the Segmenter.
    test_segmenter!(ArabicSegmenter, TEXT, SEGMENTED, TOKENIZED, Script::Arabic, Language::Ara);
}
