use crate::comment_and_docstring_extraction::DisallowedItem;
use std::cmp::Reverse;

pub fn remove_disallowed_items_from_source_text(
    source_text: &str,
    disallowed_items: &[DisallowedItem],
) -> String {
    let mut edited_source_text = source_text.to_owned();
    let mut items_from_last_to_first: Vec<&DisallowedItem> = disallowed_items.iter().collect();
    items_from_last_to_first.sort_by_key(|item| Reverse(item.start_byte_offset));
    for item in items_from_last_to_first {
        if item.replace_with_ellipsis_to_keep_body_nonempty {
            edited_source_text.replace_range(item.start_byte_offset..item.end_byte_offset, "...");
        } else {
            remove_byte_range_with_its_surrounding_layout(
                &mut edited_source_text,
                item.start_byte_offset,
                item.end_byte_offset,
            );
        }
    }
    edited_source_text
}

fn is_horizontal_whitespace(character: char) -> bool {
    character == ' ' || character == '\t'
}

fn remove_byte_range_with_its_surrounding_layout(
    text: &mut String,
    start_byte_offset: usize,
    end_byte_offset: usize,
) {
    let line_start_byte_offset = text[..start_byte_offset]
        .rfind('\n')
        .map_or(0, |index| index + 1);
    let line_end_byte_offset = text[end_byte_offset..]
        .find('\n')
        .map_or(text.len(), |index| end_byte_offset + index);
    let text_before_on_line = &text[line_start_byte_offset..start_byte_offset];
    let text_after_on_line = text[end_byte_offset..line_end_byte_offset].trim_end_matches('\r');
    let horizontal_whitespace_length_before = text_before_on_line.len()
        - text_before_on_line
            .trim_end_matches(is_horizontal_whitespace)
            .len();
    let horizontal_whitespace_length_after = text_after_on_line.len()
        - text_after_on_line
            .trim_start_matches(is_horizontal_whitespace)
            .len();
    let has_code_before_on_line = !text_before_on_line.chars().all(is_horizontal_whitespace);
    let has_code_after_on_line = !text_after_on_line.chars().all(is_horizontal_whitespace);

    match (has_code_before_on_line, has_code_after_on_line) {
        (false, false) => {
            let end_including_line_break = if line_end_byte_offset < text.len() {
                line_end_byte_offset + 1
            } else {
                line_end_byte_offset
            };
            text.replace_range(line_start_byte_offset..end_including_line_break, "");
        }
        (true, false) => {
            text.replace_range(
                start_byte_offset - horizontal_whitespace_length_before
                    ..end_byte_offset + horizontal_whitespace_length_after,
                "",
            );
        }
        (false, true) => {
            text.replace_range(
                start_byte_offset..end_byte_offset + horizontal_whitespace_length_after,
                "",
            );
        }
        (true, true) => {
            if horizontal_whitespace_length_before == 0 && horizontal_whitespace_length_after == 0 {
                text.replace_range(start_byte_offset..end_byte_offset, " ");
            } else if horizontal_whitespace_length_before > 0 {
                text.replace_range(
                    start_byte_offset..end_byte_offset + horizontal_whitespace_length_after,
                    "",
                );
            } else {
                text.replace_range(start_byte_offset..end_byte_offset, "");
            }
        }
    }
}
