use crate::compress::CompressedGotoEntry;

pub(crate) struct GotoRunCodec {
    data: Vec<CompressedGotoEntry>,
    row_offsets: Vec<u16>,
    last_state: Option<u16>,
    run_length: u16,
}

impl GotoRunCodec {
    pub(crate) fn new() -> Self {
        Self {
            data: Vec::new(),
            row_offsets: Vec::new(),
            last_state: None,
            run_length: 0,
        }
    }

    pub(crate) fn begin_row(&mut self) {
        self.row_offsets.push(self.data.len() as u16);
        self.last_state = None;
        self.run_length = 0;
    }

    pub(crate) fn push_state(&mut self, state: u16) {
        if self.last_state == Some(state) {
            self.run_length += 1;
            return;
        }

        self.emit_pending_run();
        self.last_state = Some(state);
        self.run_length = 1;
    }

    pub(crate) fn end_row(&mut self) {
        self.emit_pending_run();
    }

    pub(crate) fn finish(mut self) -> (Vec<CompressedGotoEntry>, Vec<u16>) {
        self.row_offsets.push(self.data.len() as u16);
        (self.data, self.row_offsets)
    }

    fn emit_pending_run(&mut self) {
        if self.run_length == 0 {
            return;
        }

        let state = self
            .last_state
            .expect("run_length > 0 implies last_state is set");

        if self.run_length > 2 {
            self.data.push(CompressedGotoEntry::RunLength {
                state,
                count: self.run_length,
            });
        } else {
            for _ in 0..self.run_length {
                self.data.push(CompressedGotoEntry::Single(state));
            }
        }

        self.last_state = None;
        self.run_length = 0;
    }
}
