use crate::resources::FileResource;

pub trait Reporter {
    fn report(&self, file: FileResource, status: Status, total_files: usize, total_bytes: usize);
}

pub enum Status {
    Downloaded,
    Skipped,
    Cancelled,
}
