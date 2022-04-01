
#[derive(Debug, PartialEq)]
pub enum OperationType {
    Read,
    ReadAt,
    Write,
    Mkdir,
    Mknod,
    OpenAt,
    Other
}
pub struct Operation {
    pub kind: OperationType,
    pub size: Option<usize>,
    pub path: String,
}

impl Operation {
    pub fn new(kind: OperationType, size: Option<usize>, path: String) -> Self {
        Operation {
            kind,
            size,
            path,
        }
    }


}