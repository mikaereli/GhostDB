use crate::config::ColumnStrategy;
use fake::faker::internet::en::SafeEmail;
use fake::faker::name::en::{FirstName, LastName, Name};
use fake::faker::phone_number::en::PhoneNumber;
use fake::Fake;
use rand::rngs::StdRng;
use rand::SeedableRng;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

pub struct Transformer {
    global_seed: u64,
}

impl Transformer {
    pub fn new(seed: u64) -> Self {
        Self { global_seed: seed }
    }

    pub fn transform(&self, value: &str, strategy: &ColumnStrategy) -> String {
        let is_quoted = value.starts_with('\'') && value.ends_with('\'');
        let clean_val = if is_quoted {
            &value[1..value.len() - 1]
        } else {
            value
        };

        let mut hasher = DefaultHasher::new();
        self.global_seed.hash(&mut hasher);
        clean_val.hash(&mut hasher);
        let seed = hasher.finish();
        let mut rng = StdRng::seed_from_u64(seed);

        let new_val = match strategy {
            ColumnStrategy::FirstName => FirstName().fake_with_rng(&mut rng),
            ColumnStrategy::LastName => LastName().fake_with_rng(&mut rng),
            ColumnStrategy::FullName => Name().fake_with_rng(&mut rng),
            ColumnStrategy::Email => SafeEmail().fake_with_rng(&mut rng),
            ColumnStrategy::Phone => PhoneNumber().fake_with_rng(&mut rng),
            ColumnStrategy::Mask => {
                if clean_val.contains('@') {
                    let parts: Vec<&str> = clean_val.split('@').collect();
                    if parts.len() == 2 {
                        let name = parts[0];
                        let domain = parts[1];
                        let masked_name = if name.len() > 1 {
                            format!("{}***", &name[0..1])
                        } else {
                            "***".to_string()
                        };
                        format!("{}@{}", masked_name, domain)
                    } else {
                        "***@unknown.com".to_string()
                    }
                } else {
                    if clean_val.len() > 1 {
                        format!("{}***", &clean_val[0..1])
                    } else {
                        "*".to_string()
                    }
                }
            }
            ColumnStrategy::Fixed(s) => s.clone(),
            ColumnStrategy::Keep => return value.to_string(),
        };

        if is_quoted {
            format!("'{}'", new_val)
        } else {
            new_val
        }
    }

    pub fn parse_values(values_str: &str) -> Vec<String> {
        let mut result = Vec::new();
        let mut current = String::new();
        let mut in_quotes = false;
        let mut escape = false;

        for c in values_str.chars() {
            if escape {
                current.push(c);
                escape = false;
                continue;
            }

            match c {
                '\'' => {
                    in_quotes = !in_quotes;
                    current.push(c);
                }
                '\\' => {
                    escape = true;
                    current.push(c);
                }
                ',' if !in_quotes => {
                    result.push(current.trim().to_string());
                    current.clear();
                }
                _ => {
                    current.push(c);
                }
            }
        }
        if !current.is_empty() {
            result.push(current.trim().to_string());
        }
        result
    }
}
