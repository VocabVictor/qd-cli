pub(crate) fn print_pretty_table(
    title: &str,
    headers: &[&str],
    rows: &[Vec<String>],
    max_widths: &[usize],
) {
    println!("{title}");
    if rows.is_empty() {
        println!("（没有符合条件的记录）\n");
        return;
    }

    let normalized: Vec<Vec<String>> = rows
        .iter()
        .map(|row| {
            row.iter()
                .enumerate()
                .map(|(index, value)| {
                    truncate_display(value, max_widths.get(index).copied().unwrap_or(40))
                })
                .collect()
        })
        .collect();
    let widths: Vec<usize> = headers
        .iter()
        .enumerate()
        .map(|(index, header)| {
            normalized
                .iter()
                .filter_map(|row| row.get(index))
                .map(|value| display_width(value))
                .fold(display_width(header), usize::max)
        })
        .collect();

    let header_values = headers
        .iter()
        .map(|value| (*value).to_owned())
        .collect::<Vec<_>>();
    print_table_border('┌', '┬', '┐', &widths);
    print_table_row(&header_values, &widths);
    print_table_border('├', '┼', '┤', &widths);
    for row in &normalized {
        print_table_row(row, &widths);
    }
    print_table_border('└', '┴', '┘', &widths);
    println!();
}

pub(crate) fn print_table_border(left: char, middle: char, right: char, widths: &[usize]) {
    print!("{left}");
    for (index, width) in widths.iter().enumerate() {
        print!("{}", "─".repeat(width + 2));
        print!(
            "{}",
            if index + 1 == widths.len() {
                right
            } else {
                middle
            }
        );
    }
    println!();
}

pub(crate) fn print_table_row(values: &[String], widths: &[usize]) {
    print!("│");
    for (index, width) in widths.iter().enumerate() {
        let value = values.get(index).map(String::as_str).unwrap_or("");
        let padding = width.saturating_sub(display_width(value));
        print!(" {value}{} │", " ".repeat(padding));
    }
    println!();
}

pub(crate) fn truncate_display(value: &str, max_width: usize) -> String {
    if display_width(value) <= max_width {
        return value.to_owned();
    }
    let target = max_width.saturating_sub(1);
    let mut result = String::new();
    let mut width = 0;
    for character in value.chars() {
        let character_width = character_display_width(character);
        if width + character_width > target {
            break;
        }
        result.push(character);
        width += character_width;
    }
    result.push('…');
    result
}

pub(crate) fn display_width(value: &str) -> usize {
    value.chars().map(character_display_width).sum()
}

pub(crate) fn character_display_width(character: char) -> usize {
    let value = character as u32;
    if matches!(
        value,
        0x1100..=0x115F
            | 0x2329..=0x232A
            | 0x2600..=0x27BF
            | 0x2E80..=0xA4CF
            | 0xAC00..=0xD7A3
            | 0xF900..=0xFAFF
            | 0xFE10..=0xFE19
            | 0xFE30..=0xFE6F
            | 0xFF00..=0xFF60
            | 0xFFE0..=0xFFE6
            | 0x1F300..=0x1FAFF
    ) {
        2
    } else {
        1
    }
}
