pub fn get_new_name() -> String {
    let names = petname::Petnames::default();
    let input = names.generate_one(2, " ");
    return uppercase_first_characters(input.as_str(), ' ');
}

fn uppercase_first_characters(input: &str, separator: char) -> String {
    let mut parts = input.splitn(2, separator);
    if let (Some(first), Some(second)) = (parts.next(), parts.next()) {
        return format!(
            "{} {}",
            uppercase_first_character(first),
            uppercase_first_character(second)
        );
    }

    input.to_string()
}

fn uppercase_first_character(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
}
