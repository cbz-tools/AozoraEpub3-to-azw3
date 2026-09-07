#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExthRecord {
    pub kind: u32,
    pub value: Vec<u8>,
}

#[derive(Debug, Clone, Default)]
pub struct ExthHeader {
    pub records: Vec<ExthRecord>,
}

impl ExthHeader {
    pub fn push_text(&mut self, kind: u32, value: impl AsRef<str>) {
        self.records.push(ExthRecord {
            kind,
            value: value.as_ref().as_bytes().to_vec(),
        });
    }

    pub fn push_bytes(&mut self, kind: u32, value: impl AsRef<[u8]>) {
        self.records.push(ExthRecord {
            kind,
            value: value.as_ref().to_vec(),
        });
    }

    pub fn validate(&self) -> crate::error::Result<()> {
        if self.records.len() > u32::MAX as usize {
            return Err(crate::error::Error::Output(
                "EXTH record count exceeds u32".to_owned(),
            ));
        }
        let mut length = 12usize;
        for record in &self.records {
            let record_len = 8usize.checked_add(record.value.len()).ok_or_else(|| {
                crate::error::Error::Output("EXTH record length overflow".to_owned())
            })?;
            length = length.checked_add(record_len).ok_or_else(|| {
                crate::error::Error::Output("EXTH header length overflow".to_owned())
            })?;
        }
        if length > u32::MAX as usize {
            return Err(crate::error::Error::Output(
                "EXTH header length exceeds u32".to_owned(),
            ));
        }
        Ok(())
    }

    pub fn encode_checked(&self) -> crate::error::Result<Vec<u8>> {
        self.validate()?;
        Ok(self.encode_unchecked())
    }

    fn encode_unchecked(&self) -> Vec<u8> {
        let length: usize = 12
            + self
                .records
                .iter()
                .map(|record| 8 + record.value.len())
                .sum::<usize>();
        let mut bytes = Vec::with_capacity(length);
        bytes.extend_from_slice(b"EXTH");
        bytes.extend_from_slice(&(length as u32).to_be_bytes());
        bytes.extend_from_slice(&(self.records.len() as u32).to_be_bytes());
        for record in &self.records {
            bytes.extend_from_slice(&record.kind.to_be_bytes());
            bytes.extend_from_slice(&((8 + record.value.len()) as u32).to_be_bytes());
            bytes.extend_from_slice(&record.value);
        }
        bytes
    }
}
