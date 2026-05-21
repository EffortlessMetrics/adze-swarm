use crate::{
    compress::CompressedGotoEntry,
    error::{Result, TableGenError},
};

pub(crate) struct GotoRunCodec {
    data: Vec<CompressedGotoEntry>,
    row_offsets: Vec<u16>,
    last_state: Option<u16>,
    run_length: usize,
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

    pub(crate) fn begin_row(&mut self) -> Result<()> {
        self.row_offsets
            .push(checked_u16(self.data.len(), "goto row offset")?);
        self.last_state = None;
        self.run_length = 0;
        Ok(())
    }

    pub(crate) fn push_state(&mut self, state: u16) -> Result<()> {
        if self.last_state == Some(state) {
            self.run_length += 1;
            return Ok(());
        }

        self.emit_pending_run()?;
        self.last_state = Some(state);
        self.run_length = 1;
        Ok(())
    }

    pub(crate) fn end_row(&mut self) -> Result<()> {
        self.emit_pending_run()
    }

    pub(crate) fn finish(mut self) -> Result<(Vec<CompressedGotoEntry>, Vec<u16>)> {
        self.row_offsets
            .push(checked_u16(self.data.len(), "goto row offset")?);
        Ok((self.data, self.row_offsets))
    }

    fn emit_pending_run(&mut self) -> Result<()> {
        if self.run_length == 0 {
            return Ok(());
        }

        let state = self
            .last_state
            .expect("run_length > 0 implies last_state is set");

        if self.run_length > 2 {
            self.data.push(CompressedGotoEntry::RunLength {
                state,
                count: checked_u16(self.run_length, "goto run length")?,
            });
        } else {
            for _ in 0..self.run_length {
                self.data.push(CompressedGotoEntry::Single(state));
            }
        }

        self.last_state = None;
        self.run_length = 0;
        Ok(())
    }
}

fn checked_u16(value: usize, context: &'static str) -> Result<u16> {
    u16::try_from(value).map_err(|_| {
        TableGenError::Compression(format!(
            "{context} overflow: {value} exceeds u16::MAX ({})",
            u16::MAX
        ))
    })
}
