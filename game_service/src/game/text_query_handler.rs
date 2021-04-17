pub struct TextQueryHandler {
    texts: Vec<String>,
}

impl TextQueryHandler {
    pub fn new(texts: Vec<String>) -> Self {
        Self { texts }
    }

    pub fn query(&self, query: &str, page_size: usize, skip: usize) -> (Vec<String>, bool) {
        let mut texts: Vec<&str> = (self.texts)
            .iter()
            .filter(|text| text.contains(query))
            .skip(skip)
            .take(page_size + 1)
            .map(|text| &text[..])
            .collect();
        let has_next_page = texts.len() == page_size + 1;
        if has_next_page {
            texts.pop();
        }
        (
            texts.iter().map(|text| String::from(*text)).collect(),
            has_next_page,
        )
    }

    pub fn total_size(&self) -> usize {
        self.texts.len()
    }
}
