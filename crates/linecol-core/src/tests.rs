use crate::LineCol;

#[test]
fn basic_newline_tracking() {
    let input = b"hello\nworld\n";
    let tracker = LineCol::at_position(input, 6);
    assert_eq!(tracker.line, 1);
    assert_eq!(tracker.line_start, 6);
    assert_eq!(tracker.column(8), 2);
}

#[test]
fn crlf_handling() {
    let input = b"hello\r\nworld\r\n";
    let tracker = LineCol::at_position(input, 7);
    assert_eq!(tracker.line, 1);
    assert_eq!(tracker.line_start, 7);
    assert_eq!(tracker.column(9), 2);
}

#[test]
fn cr_only_handling() {
    let input = b"hello\rworld\r";
    let tracker = LineCol::at_position(input, 6);
    assert_eq!(tracker.line, 1);
    assert_eq!(tracker.line_start, 6);
}

#[test]
fn process_byte_tracks_line_boundaries() {
    let mut tracker = LineCol::new();

    assert!(!tracker.process_byte(b'a', None, 0));
    assert_eq!(tracker.line, 0);

    assert!(tracker.process_byte(b'\n', None, 5));
    assert_eq!(tracker.line, 1);
    assert_eq!(tracker.line_start, 6);

    assert!(tracker.process_byte(b'\r', Some(b'x'), 10));
    assert_eq!(tracker.line, 2);
    assert_eq!(tracker.line_start, 11);

    assert!(!tracker.process_byte(b'\r', Some(b'\n'), 15));
    assert_eq!(tracker.line, 2);
}

#[test]
fn column_at_line_start_is_zero() {
    let tracker = LineCol::at_position(b"ab\ncd", 3);
    assert_eq!(tracker.column(3), 0);
}

#[test]
fn column_saturates_when_position_below_line_start() {
    let tracker = LineCol {
        line: 1,
        line_start: 10,
    };
    assert_eq!(tracker.column(5), 0);
}

#[test]
fn advance_line_increments_line_by_exactly_one() {
    let mut tracker = LineCol::new();
    tracker.advance_line(5);
    assert_eq!(tracker.line, 1);
    assert_eq!(tracker.line_start, 5);
    tracker.advance_line(10);
    assert_eq!(tracker.line, 2);
    assert_eq!(tracker.line_start, 10);
}

#[test]
fn at_position_zero_returns_initial_state() {
    let tracker = LineCol::at_position(b"hello\nworld", 0);
    assert_eq!(tracker.line, 0);
    assert_eq!(tracker.line_start, 0);
}

#[test]
fn at_position_just_past_newline() {
    let input = b"a\nb";
    let before = LineCol::at_position(input, 1);
    let after = LineCol::at_position(input, 2);
    assert_eq!(before.line, 0);
    assert_eq!(after.line, 1);
    assert_eq!(after.line_start, 2);
}

#[test]
fn multiple_consecutive_newlines() {
    let input = b"\n\n\n";
    let tracker = LineCol::at_position(input, 3);
    assert_eq!(tracker.line, 3);
    assert_eq!(tracker.line_start, 3);
}

#[test]
fn new_fields_are_zero() {
    let tracker = LineCol::new();
    assert_eq!(tracker.line, 0);
    assert_eq!(tracker.line_start, 0);
}
