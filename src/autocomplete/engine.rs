use super::extractor::{SymbolKind, UserSymbol, extract_user_labels};
use super::intel8080_docs::{INTEL_8080_DOCS, InstructionDoc};
use slint::SharedString;

#[derive(Clone, Debug)]
pub struct Suggestion {
    pub name: SharedString,
    pub category: SharedString,
    pub syntax: SharedString,
    pub summary: SharedString,
    pub flags: SharedString,
    pub cycles: SharedString,
    pub insert_text: SharedString,
    pub icon_kind: SharedString,
    pub example: SharedString,
}

#[derive(Clone, Debug)]
pub struct AutocompleteEngine {
    docs: &'static [InstructionDoc],
}

impl AutocompleteEngine {
    pub fn new() -> Self {
        Self {
            docs: INTEL_8080_DOCS,
        }
    }

    #[allow(dead_code)]
    pub fn get_suggestions(&self, current_word: &str, full_source: &str) -> Vec<Suggestion> {
        self.get_suggestions_lang(current_word, full_source, "en")
    }

    pub fn get_suggestions_lang(
        &self,
        current_word: &str,
        full_source: &str,
        lang: &str,
    ) -> Vec<Suggestion> {
        let mut results = Vec::with_capacity(32);
        let query = current_word.trim();
        let is_ru = lang == "ru";
        let query_upper = query.to_ascii_uppercase();

        let mut doc_results = Vec::with_capacity(self.docs.len());
        for doc in self.docs {
            if query.is_empty() || doc.name.starts_with(&query_upper) {
                doc_results.push(Suggestion {
                    name: SharedString::from(doc.name),
                    category: SharedString::from(doc.get_category(lang)),
                    syntax: SharedString::from(doc.syntax),
                    summary: SharedString::from(doc.get_summary(lang)),
                    flags: SharedString::from(doc.flags),
                    cycles: SharedString::from(doc.cycles),
                    insert_text: SharedString::from(doc.name.to_ascii_lowercase().as_str()),
                    icon_kind: SharedString::from(doc.get_icon_kind()),
                    example: SharedString::from(doc.get_example()),
                });
            }
        }

        let user_symbols = extract_user_labels(full_source);
        let mut user_results = Vec::with_capacity(user_symbols.len());
        for symbol in &user_symbols {
            if query.is_empty() || symbol.name.to_ascii_uppercase().starts_with(&query_upper) {
                let (category, summary, example) = match symbol.kind {
                    SymbolKind::Constant => {
                        let cat = if is_ru {
                            SharedString::from("Константа (EQU)")
                        } else {
                            SharedString::from("Constant (EQU)")
                        };
                        let sum = if is_ru {
                            SharedString::from(format!(
                                "Символьная константа EQU ({}, строка {})",
                                symbol.detail, symbol.line_number
                            ))
                        } else {
                            SharedString::from(format!(
                                "Symbolic EQU constant ({}, line {})",
                                symbol.detail, symbol.line_number
                            ))
                        };
                        let ex = SharedString::from(format!(
                            "mvi a, {}    ; Загрузка константы {}",
                            symbol.name, symbol.name
                        ));
                        (cat, sum, ex)
                    }
                    SymbolKind::Data => {
                        let cat = if is_ru {
                            SharedString::from("Данные / Буфер")
                        } else {
                            SharedString::from("Data / Buffer")
                        };
                        let sum = if is_ru {
                            SharedString::from(format!(
                                "Определение данных в памяти ({}, строка {})",
                                symbol.detail, symbol.line_number
                            ))
                        } else {
                            SharedString::from(format!(
                                "Memory data definition ({}, line {})",
                                symbol.detail, symbol.line_number
                            ))
                        };
                        let ex = SharedString::from(format!(
                            "lxi d, {}    ; Адрес буфера {}",
                            symbol.name, symbol.name
                        ));
                        (cat, sum, ex)
                    }
                    SymbolKind::Code => {
                        let cat = if is_ru {
                            SharedString::from("Метка кода")
                        } else {
                            SharedString::from("Code Label")
                        };
                        let sum = if is_ru {
                            SharedString::from(format!(
                                "Метка перехода / подпрограммы (строка {})",
                                symbol.line_number
                            ))
                        } else {
                            SharedString::from(format!(
                                "Code label / jump target (line {})",
                                symbol.line_number
                            ))
                        };
                        let ex = SharedString::from(format!(
                            "jmp {}        ; Переход на метку {}",
                            symbol.name, symbol.name
                        ));
                        (cat, sum, ex)
                    }
                };

                let sym_shared = SharedString::from(symbol.name.as_str());
                user_results.push(Suggestion {
                    name: sym_shared.clone(),
                    category,
                    syntax: sym_shared.clone(),
                    summary,
                    flags: if is_ru {
                        SharedString::from("Нет")
                    } else {
                        SharedString::from("None")
                    },
                    cycles: SharedString::from("-"),
                    insert_text: sym_shared,
                    icon_kind: SharedString::from("label"),
                    example,
                });
            }
        }

        if query.is_empty() {
            results.extend(doc_results);
            results.extend(user_results);
        } else {
            let mut exact_matches = Vec::with_capacity(2);
            let mut other_matches = Vec::with_capacity(doc_results.len() + user_results.len());

            for s in doc_results.into_iter().chain(user_results) {
                if s.name.eq_ignore_ascii_case(query) {
                    exact_matches.push(s);
                } else {
                    other_matches.push(s);
                }
            }

            results.extend(exact_matches);
            results.extend(other_matches);
        }

        if !query.is_empty() && results.is_empty() {
            let upper = query.to_ascii_uppercase();
            for doc in self.docs {
                if doc.name.contains(&upper)
                    || doc.get_summary(lang).to_ascii_uppercase().contains(&upper)
                {
                    results.push(Suggestion {
                        name: SharedString::from(doc.name),
                        category: SharedString::from(doc.get_category(lang)),
                        syntax: SharedString::from(doc.syntax),
                        summary: SharedString::from(doc.get_summary(lang)),
                        flags: SharedString::from(doc.flags),
                        cycles: SharedString::from(doc.cycles),
                        insert_text: SharedString::from(doc.name.to_ascii_lowercase().as_str()),
                        icon_kind: SharedString::from(doc.get_icon_kind()),
                        example: SharedString::from(doc.get_example()),
                    });
                }
            }
        }

        results
    }

    pub fn get_user_labels(&self, full_source: &str) -> Vec<UserSymbol> {
        extract_user_labels(full_source)
    }

    #[allow(dead_code)]
    pub fn find_doc(&self, instruction: &str) -> Option<&'static InstructionDoc> {
        let upper = instruction.trim().to_ascii_uppercase();
        self.docs.iter().find(|d| d.name == upper)
    }
}
