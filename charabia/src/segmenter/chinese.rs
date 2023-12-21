use jieba_rs::Jieba;
use jieba_rs::{TokenizeMode, Token as JiebaToken };
use once_cell::sync::Lazy;
use std::fs::File;
use std::io::{self, BufRead};
use std::path::Path;

use crate::segmenter:: {Segmenter, TokenItem};

/// Chinese Script specialized [`Segmenter`].
///
/// This Segmenter uses [`Jieba`] internally to segment the provided text
/// with HMM feature.
pub struct ChineseSegmenter;

impl Segmenter for ChineseSegmenter {
    fn segment_str<'o>(&self, to_segment: &'o str) -> Box<dyn Iterator<Item = TokenItem<'o> > + 'o> {
        let token_items: Vec<TokenItem> = JIEBA.tokenize(to_segment, TokenizeMode::Search, true)
            .iter()
            .map(|token| TokenItem::WithPosition { 
                text: token.word, 
                char_start: token.start, 
                char_end: token.end,
                byte_start: token.byte_start,
                byte_end: token.byte_end
            })
            .collect();
        Box::new(token_items.into_iter())
    }
}

fn read_lines<P>(filename: P) -> Vec<String>
where
    P: AsRef<Path>,
{
    let path = filename.as_ref();
    if !path.exists() {
        println!("****");
        return vec![];
    }

    if let Ok(file) = File::open(&path) {
        let reader = io::BufReader::new(file);
        let mut lines = Vec::new();

        for line in reader.lines() {
            if let Ok(line) = line {
                lines.push(line);
            }
        }

        return lines;
    }
    return vec![];
}

static JIEBA: Lazy<Jieba> = Lazy::new(|| {
    let mut jieba = Jieba::new();
    let lines = read_lines("./words.txt");
    for line in lines {
        jieba.add_word(line.as_str(), Some(99 as usize), None);
    }
    jieba
});

#[cfg(test)]
mod test {
    use crate::segmenter::test::test_segmenter;

    // Original version of the text.
    const TEXT: &str =
    "尊嚴割草机器人压缩饼干上海盛赟实业河北春高贸易上海椒龙数码深圳市宏业盛科技百事甜纺织湖北山东雅云卫生用品有限公司空气压缩机";

    // Segmented version of the text.
    const SEGMENTED: &[&str] = &["尊嚴","割草", "机器", "机器人", "压缩", "饼干", "压缩饼干", "上海", "盛赟", "实业", "河北", "春高", "贸易", "上海", "椒", "龙", "数码", "深圳", "深圳市", "宏业", "盛", "科技", "百事", "甜", "纺织", "湖北", "山东", "雅云", "卫生", "用品", "卫生用品", "有限", "公司", "有限公司", "空气", "气压", "压缩", "压缩机", "空气压缩机"];

    // Segmented and normalized version of the text.
    const TOKENIZED: &[&str] = &["尊严", "割草", "机器", "机器人", "压缩", "饼干", "压缩饼干", "上海", "盛赟", "实业", "河北", "春高", "贸易", "上海", "椒", "龙", "数码","深圳", "深圳市", "宏业", "盛", "科技", "百事", "甜", "纺织", "湖北", "山东", "雅云", "卫生", "用品", "卫生用品", "有限", "公司", "有限公司", "空气", "气压", "压缩", "压缩机", "空气压缩机"];

    // Macro that run several tests on the Segmenter.
    test_segmenter!(ChineseSegmenter, TEXT, SEGMENTED, TOKENIZED, Script::Cj, Language::Cmn);
}
